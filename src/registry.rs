//! Durable project/workspace identity. Only the existing storage worker calls SQL.
use crate::{events::Observation, git::CheckoutIdentity};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub comparison_bases: std::collections::BTreeMap<String, crate::working_diff::Base>,
    pub projects: Vec<Project>,
    pub workspaces: Vec<Workspace>,
    pub selected: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub common_dir: Vec<u8>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    /// Original opaque task identity, never parsed or rewritten during migration.
    pub task: String,
    pub legacy_tasks: Vec<String>,
    pub name: String,
    pub project: Option<String>,
    pub path: Option<Vec<u8>>,
    pub checkout: Option<CheckoutIdentity>,
    pub sessions: Vec<String>,
}
impl Registry {
    pub fn checkout(&self, identity: &CheckoutIdentity) -> Option<&Workspace> {
        self.workspaces.iter().find(|workspace| {
            workspace.checkout.as_ref().is_some_and(|known| {
                known.common_dir == identity.common_dir && known.git_dir == identity.git_dir
            })
        })
    }
}

/// Local label from supplied work. This is never a native conversation title.
pub fn prompt_name(prompt: &str) -> Option<String> {
    let clean = prompt
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.is_empty() {
        return None;
    }
    let mut label = clean.chars().take(72).collect::<String>();
    if clean.chars().count() > 72 {
        label.push('…');
    }
    Some(label)
}

pub(crate) fn rename(db: &Connection, workspace: &str, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() || name.len() > 240 || name.chars().any(char::is_control) {
        return Err("Workspace name must contain 1–240 bytes without control characters".into());
    }
    if db.execute(
        "UPDATE workspaces SET name=?1 WHERE id=?2",
        params![name, workspace],
    )? != 1
    {
        return Err("Workspace unavailable; refresh before renaming".into());
    }
    db.execute(
        "UPDATE workspace_names SET source='manual' WHERE workspace=?1",
        [workspace],
    )?;
    Ok(())
}

pub(crate) fn name_from_prompt(db: &Connection, task: &str, name: &str) -> Result<()> {
    // A first-prompt default may replace only the original generated fallback.
    // Manual edits and names from a previous conversation always win.
    db.execute("UPDATE workspaces SET name=?1 WHERE id=(SELECT workspace FROM workspace_tasks WHERE task=?2) AND id IN (SELECT workspace FROM workspace_names WHERE source='fallback')", params![name,task])?;
    db.execute("UPDATE workspace_names SET source='prompt' WHERE workspace=(SELECT workspace FROM workspace_tasks WHERE task=?1) AND source='fallback'", [task])?;
    Ok(())
}

pub(crate) fn create(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE projects(id TEXT PRIMARY KEY, common_dir BLOB NOT NULL UNIQUE, name TEXT NOT NULL);
        CREATE TABLE workspaces(id TEXT PRIMARY KEY, task TEXT NOT NULL UNIQUE, name TEXT NOT NULL, project TEXT, path BLOB, checkout TEXT, common_dir BLOB, git_dir BLOB, UNIQUE(common_dir,git_dir));
        CREATE TABLE workspace_tasks(task TEXT PRIMARY KEY,workspace TEXT NOT NULL);
        CREATE TABLE workspace_sessions(session TEXT PRIMARY KEY, workspace TEXT NOT NULL);")?;
    Ok(())
}

pub(crate) fn record(
    db: &Connection,
    state: &Observation,
    path: Option<&[u8]>,
    task: &str,
) -> Result<()> {
    // Empty initial state is not a workspace. Explicit path or session membership is required.
    if path.is_none() && state.session_identity.is_none() {
        return Ok(());
    }
    let known = state.checkout_identity.as_ref();
    let recorded_root = known.map(|checkout| crate::session::path_bytes(&checkout.root));
    let path = path.or(recorded_root.as_deref());
    let project = if let Some(checkout) = known {
        let common = crate::session::path_bytes(&checkout.common_dir);
        let id: Option<String> = db
            .query_row(
                "SELECT id FROM projects WHERE common_dir=?1",
                [&common],
                |row| row.get(0),
            )
            .optional()?;
        let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let name = checkout
            .root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        db.execute(
            "INSERT INTO projects VALUES(?1,?2,?3) ON CONFLICT(common_dir) DO NOTHING",
            params![id, common, name],
        )?;
        Some(id)
    } else {
        None
    };
    let existing: Option<String> = db
        .query_row(
            "SELECT workspace FROM workspace_tasks WHERE task=?1",
            [task],
            |row| row.get(0),
        )
        .optional()?;
    let common = known.map(|checkout| crate::session::path_bytes(&checkout.common_dir));
    let git_dir = known.map(|checkout| crate::session::path_bytes(&checkout.git_dir));
    if let (Some(id), Some(common), Some(git_dir)) = (&existing, &common, &git_dir) {
        let previous: (Option<Vec<u8>>, Option<Vec<u8>>) = db.query_row(
            "SELECT common_dir,git_dir FROM workspaces WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if previous.0.as_ref().is_some_and(|old| old != common)
            || previous.1.as_ref().is_some_and(|old| old != git_dir)
        {
            return Err(
                "Retained task has conflicting canonical checkout facts; association unresolved"
                    .into(),
            );
        }
    }
    let canonical: Option<String> = if let (Some(common), Some(git_dir)) = (&common, &git_dir) {
        db.query_row(
            "SELECT id FROM workspaces WHERE common_dir=?1 AND git_dir=?2",
            params![common, git_dir],
            |row| row.get(0),
        )
        .optional()?
    } else {
        None
    };
    if existing
        .as_ref()
        .zip(canonical.as_ref())
        .is_some_and(|(old, new)| old != new)
    {
        return Err("Retained task and canonical checkout identify different workspaces; association unresolved".into());
    }
    let id = existing
        .or(canonical)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let name = format!("Workspace {}", &id[..8]);
    let checkout = known.map(serde_json::to_string).transpose()?;
    db.execute("INSERT INTO workspaces VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET project=coalesce(excluded.project,workspaces.project),path=coalesce(excluded.path,workspaces.path),checkout=coalesce(excluded.checkout,workspaces.checkout),common_dir=coalesce(excluded.common_dir,workspaces.common_dir),git_dir=coalesce(excluded.git_dir,workspaces.git_dir)", params![id,task,name,project,path,checkout,common,git_dir])?;
    db.execute(
        "INSERT INTO workspace_names VALUES(?1,'fallback') ON CONFLICT(workspace) DO NOTHING",
        [&id],
    )?;
    db.execute(
        "INSERT INTO workspace_tasks VALUES(?1,?2) ON CONFLICT(task) DO NOTHING",
        params![task, id],
    )?;
    if let Some(session) = &state.session_identity {
        if session.task != task {
            return Err("Session task conflicts with retained workspace".into());
        }
        db.execute(
            "INSERT INTO workspace_sessions VALUES(?1,?2) ON CONFLICT(session) DO NOTHING",
            params![session.session, id],
        )?;
        let actual: String = db.query_row(
            "SELECT workspace FROM workspace_sessions WHERE session=?1",
            [&session.session],
            |row| row.get(0),
        )?;
        if actual != id {
            return Err("Session already belongs to another workspace".into());
        }
    }
    Ok(())
}

pub(crate) fn snapshot(db: &Connection, selected_task: Option<&str>) -> Result<Registry> {
    let projects = db
        .prepare("SELECT id,name,common_dir FROM projects ORDER BY rowid")?
        .query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                common_dir: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let rows = db
        .prepare("SELECT id,task,name,project,path,checkout FROM workspaces ORDER BY rowid")?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<Vec<u8>>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut workspaces = Vec::new();
    for (id, task, name, project, path, checkout) in rows {
        let sessions = db
            .prepare("SELECT session FROM workspace_sessions WHERE workspace=?1 ORDER BY rowid")?
            .query_map([&id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let legacy_tasks = db
            .prepare("SELECT task FROM workspace_tasks WHERE workspace=?1 ORDER BY rowid")?
            .query_map([&id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        workspaces.push(Workspace {
            legacy_tasks,
            id,
            task,
            name,
            project,
            path,
            checkout: checkout.as_deref().map(serde_json::from_str).transpose()?,
            sessions,
        });
    }
    let selected = selected_task
        .map(|task| {
            db.query_row(
                "SELECT workspace FROM workspace_tasks WHERE task=?1",
                [task],
                |row| row.get::<_, String>(0),
            )
            .optional()
        })
        .transpose()?
        .flatten();
    let comparison_bases = db
        .prepare("SELECT workspace,reference,base_commit FROM workspace_comparison_bases")?
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                crate::working_diff::Base {
                    reference: row.get(1)?,
                    commit: row.get(2)?,
                },
            ))
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(Registry {
        comparison_bases,
        projects,
        workspaces,
        selected,
    })
}
