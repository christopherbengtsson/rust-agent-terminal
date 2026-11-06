//! Bounded retained-history browser, derivative projection and exact-word index.
use super::*;
use std::collections::BTreeSet;
const VERSION: u32 = 1;
const ROWS: usize = 32;
const SLICE: u64 = 512;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Workspaces,
    Sessions,
    History,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PassageId {
    pub session: String,
    pub version: u32,
    pub ordinal: u64,
    pub raw_start: u64,
    pub raw_end: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Query {
    pub request: String,
    pub mode: Mode,
    pub text: String,
    pub after: i64,
    pub workspace: Option<String>,
    pub passage: Option<PassageId>,
    pub cancel: bool,
}
impl Query {
    pub fn new(mode: Mode, text: &str) -> Self {
        Self {
            request: uuid::Uuid::new_v4().to_string(),
            mode,
            text: text.into(),
            after: 0,
            workspace: None,
            passage: None,
            cancel: false,
        }
    }
    pub fn open(request: &str, passage: PassageId) -> Self {
        Self {
            request: request.into(),
            passage: Some(passage),
            ..Self::new(Mode::History, "")
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Row {
    pub key: String,
    pub workspace: String,
    pub session: Option<String>,
    pub label: String,
    pub snippet: String,
    pub passage: Option<PassageId>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Response {
    pub request: String,
    pub rows: Vec<Row>,
    pub after: i64,
    pub more: bool,
    pub preparing: bool,
    pub opened: Option<crate::history::ReadablePage>,
    pub error: Option<String>,
}
pub(super) struct Job {
    query: Query,
    response: Response,
    terms: Vec<String>,
    upper: i64,
}
pub(super) fn schema(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE history_projection(session TEXT PRIMARY KEY,version INTEGER NOT NULL,frontier INTEGER NOT NULL,ordinal INTEGER NOT NULL,checkpoint TEXT NOT NULL,sealed INTEGER NOT NULL);
    CREATE TABLE history_passages(id INTEGER PRIMARY KEY AUTOINCREMENT,session TEXT NOT NULL,version INTEGER NOT NULL,ordinal INTEGER NOT NULL,raw_start INTEGER NOT NULL,raw_end INTEGER NOT NULL,text TEXT NOT NULL,UNIQUE(session,version,ordinal));
    CREATE TABLE history_terms(term TEXT NOT NULL,passage INTEGER NOT NULL,PRIMARY KEY(term,passage)) WITHOUT ROWID;
    CREATE INDEX history_passages_session ON history_passages(session,id);
    CREATE INDEX history_terms_passage ON history_terms(passage,term);
    CREATE TABLE workspace_history_facts(workspace TEXT PRIMARY KEY,archived INTEGER NOT NULL DEFAULT 0);
    CREATE TABLE checkout_branch_facts(path BLOB PRIMARY KEY,branch TEXT NOT NULL);")?;
    Ok(())
}
fn terms(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
pub fn visible(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
                '�'
            } else {
                c
            }
        })
        .collect()
}
fn clipped(text: &str, query: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let position = text.to_lowercase().find(query).unwrap_or(0);
    let at = text[..text.floor_char_boundary(position.min(text.len()))]
        .chars()
        .count()
        .saturating_sub(50);
    let mut result: String = chars.iter().skip(at).take(240).collect();
    if at > 0 {
        result.insert(0, '…');
    }
    if chars.len() > at + 240 {
        result.push('…');
    }
    visible(&result)
}
impl Store {
    pub(super) fn start_browse(&mut self, query: Query) -> search::Response {
        self.search_job = None;
        let mut response = Response {
            error: self.index_error.clone(),
            request: query.request.clone(),
            after: query.after.max(0),
            ..Default::default()
        };
        if query.cancel {
            return response;
        }
        let result = (|| -> Result<Option<Job>> {
            if query.request.len() > 80
                || query.text.len() > 256
                || query.workspace.as_ref().is_some_and(|s| s.len() > 80)
            {
                return Err("Search request exceeds bounded size".into());
            }
            if let Some(passage) = &query.passage {
                if passage.version != VERSION {
                    return Err("Readable passage version changed; repeat search".into());
                }
                let (start,end,text,durable): (u64,u64,String,u64) = self.db.query_row("SELECT p.raw_start,p.raw_end,p.text,s.durable FROM history_passages p JOIN sessions s ON s.id=p.session WHERE p.session=?1 AND p.version=?2 AND p.ordinal=?3 AND s.history_state='retained'", params![passage.session,passage.version,passage.ordinal], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?.ok_or("Selected passage unavailable or history deleted")?;
                if start != passage.raw_start || end != passage.raw_end || end > durable {
                    return Err("Selected passage changed; repeat search".into());
                }
                response.opened = Some(crate::history::ReadablePage {
                    version: passage.version,
                    raw_start: start,
                    raw_end: end,
                    committed: durable,
                    passages: vec![crate::history::Passage {
                        raw_start: start,
                        raw_end: end,
                        text_start: 0,
                        text_end: text.len(),
                        provisional: false,
                    }],
                    text,
                });
                return Ok(None);
            }
            let tokens = terms(&query.text);
            if query.mode == Mode::History && tokens.is_empty() {
                return Err("Enter words to search committed readable history".into());
            }
            if tokens.len() > 8 {
                return Err("Search supports at most eight words".into());
            }
            let table = match query.mode {
                Mode::History => "history_passages",
                Mode::Sessions => "sessions",
                Mode::Workspaces => "workspaces",
            };
            let upper = self.db.query_row(
                &format!("SELECT coalesce(max(rowid),0) FROM {table}"),
                [],
                |r| r.get(0),
            )?;
            response.preparing = true;
            Ok(Some(Job {
                query,
                response: response.clone(),
                terms: tokens,
                upper,
            }))
        })();
        match result {
            Ok(job) => self.search_job = job,
            Err(error) => response.error = Some(error.to_string()),
        }
        response
    }
    pub(super) fn browse_step(&mut self) -> Option<Completion> {
        let mut job = self.search_job.take()?;
        let result = self.browse_slice(&mut job);
        match result {
            Ok(false) => {
                self.search_job = Some(job);
                None
            }
            result => {
                job.response.preparing = false;
                if let Err(error) = result {
                    job.response.error = Some(error.to_string());
                }
                Some(Completion::Browse(job.response))
            }
        }
    }
    fn browse_slice(&self, job: &mut Job) -> Result<bool> {
        let after = job.response.after;
        let ids: Vec<i64> = match job.query.mode {
            Mode::History => self.db.prepare("SELECT passage FROM history_terms WHERE term=?1 AND passage>?2 AND passage<=?3 ORDER BY passage LIMIT 32")?.query_map(params![job.terms[0],after,job.upper], |r| r.get(0))?.collect::<std::result::Result<_,_>>()?,
            mode => self.db.prepare(&format!("SELECT rowid FROM {} WHERE rowid>?1 AND rowid<=?2 ORDER BY rowid LIMIT 32", if mode==Mode::Sessions {"sessions"} else {"workspaces"}))?.query_map(params![after,job.upper], |r| r.get(0))?.collect::<std::result::Result<_,_>>()?,
        };
        let exhausted = ids.len() < 32;
        for id in ids {
            job.response.after = id;
            let (workspace, session, passage, text) = match job.query.mode {
                Mode::Workspaces => {
                    let workspace: String = self.db.query_row(
                        "SELECT id FROM workspaces WHERE rowid=?1",
                        [id],
                        |r| r.get(0),
                    )?;
                    (workspace, None, None, String::new())
                }
                Mode::Sessions => {
                    let (session,workspace): (String,String)=self.db.query_row("SELECT s.id,coalesce(ws.workspace,'') FROM sessions s LEFT JOIN workspace_sessions ws ON ws.session=s.id WHERE s.rowid=?1",[id],|r|Ok((r.get(0)?,r.get(1)?)))?;
                    (workspace, Some(session), None, String::new())
                }
                Mode::History => {
                    let record: Option<(String,String,u32,u64,u64,u64,String)>=self.db.query_row("SELECT coalesce(ws.workspace,''),p.session,p.version,p.ordinal,p.raw_start,p.raw_end,p.text FROM history_passages p JOIN sessions s ON s.id=p.session LEFT JOIN workspace_sessions ws ON ws.session=p.session WHERE p.id=?1 AND p.version=?2 AND s.history_state='retained' AND p.raw_end<=s.durable",params![id,VERSION],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).optional()?;
                    let Some((workspace, session, version, ordinal, raw_start, raw_end, text)) =
                        record
                    else {
                        continue;
                    };
                    let tokens = terms(&text);
                    if !job.terms.iter().all(|term| tokens.contains(term)) {
                        continue;
                    }
                    let passage = PassageId {
                        session: session.clone(),
                        version,
                        ordinal,
                        raw_start,
                        raw_end,
                    };
                    (workspace, Some(session), Some(passage), text)
                }
            };
            if job
                .query
                .workspace
                .as_ref()
                .is_some_and(|w| w != &workspace)
            {
                continue;
            }
            let label = self.browse_label(&workspace, session.as_deref())?;
            if job.query.mode != Mode::History
                && !job.query.text.is_empty()
                && !label
                    .to_lowercase()
                    .contains(&job.query.text.to_lowercase())
            {
                continue;
            }
            let key = passage.as_ref().map_or_else(
                || session.clone().unwrap_or_else(|| workspace.clone()),
                |p| format!("{}:{}:{}", p.session, p.version, p.ordinal),
            );
            let label = label.chars().take(1000).collect();
            job.response.rows.push(Row {
                key,
                workspace,
                session,
                label,
                snippet: clipped(&text, job.terms.first().map_or("", String::as_str)),
                passage,
            });
            if job.response.rows.len() == ROWS {
                job.response.more = true;
                return Ok(true);
            }
        }
        Ok(exhausted)
    }
    fn browse_label(&self, workspace: &str, session: Option<&str>) -> Result<String> {
        type WorkspaceFacts = (String, Option<Vec<u8>>, String, bool, Option<String>);
        let facts: Option<WorkspaceFacts>=self.db.query_row("SELECT w.name,w.path,coalesce(p.name,'project unknown'),coalesce(f.archived,0),b.branch FROM workspaces w LEFT JOIN projects p ON p.id=w.project LEFT JOIN workspace_history_facts f ON f.workspace=w.id LEFT JOIN checkout_branch_facts b ON b.path=w.path WHERE w.id=?1",[workspace],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
        let mut label = if let Some((name, path, project, archived, branch)) = facts {
            let (path, status) = path.map_or_else(
                || ("path unknown".into(), "checkout unknown"),
                |raw| {
                    let path = PathBuf::from(std::ffi::OsString::from_vec(raw));
                    let status = match std::fs::metadata(&path) {
                        Ok(_) => "checkout present",
                        Err(e) if e.kind() == io::ErrorKind::NotFound => "checkout missing",
                        Err(_) => "checkout inaccessible",
                    };
                    (path.display().to_string(), status)
                },
            );
            format!(
                "{project} · {name} · {} · {path} · {status}{}",
                branch.map_or("branch unknown (legacy/unobserved)".into(), |b| format!(
                    "last observed branch {b}"
                )),
                if archived { " · archived" } else { "" }
            )
        } else {
            "workspace unknown (legacy)".into()
        };
        if let Some(session) = session {
            let (state, history): (String, String) = self.db.query_row(
                "SELECT state,history_state FROM sessions WHERE id=?1",
                [session],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let state = decode_record(&state)?;
            label = format!(
                "history {history} · {:?} · {:?} · {session} · {label}",
                state.agent, state.lifecycle
            );
        }
        Ok(visible(&label))
    }
    /// One retained row and <=512 raw bytes per pass. Checkpoint and passages commit together.
    pub(super) fn index_step(&mut self) -> Result<bool> {
        let next:Option<(i64,String,u64,bool)>=self.db.query_row("SELECT rowid,id,durable,sealed FROM sessions WHERE rowid>?1 ORDER BY rowid LIMIT 1",[self.index_cursor],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let Some((row, id, durable, sealed)) = next else {
            self.index_cursor = 0;
            return Ok(false);
        };
        self.index_cursor = row;
        let retained: bool = self.db.query_row(
            "SELECT history_state='retained' FROM sessions WHERE id=?1",
            [&id],
            |r| r.get(0),
        )?;
        if !retained {
            return self.prune_index(&id);
        }
        // A changed projection version rebuilds from authoritative committed raw.
        // The composite unique index seeks one older passage; cleanup never scans a prefix.
        let obsolete:Option<i64>=self.db.query_row("SELECT id FROM history_passages WHERE session=?1 AND version<?2 ORDER BY version,ordinal LIMIT 1",params![id,VERSION],|r|r.get(0)).optional()?;
        if let Some(passage) = obsolete {
            let tx = self.db.transaction()?;
            tx.execute("DELETE FROM history_terms WHERE passage=?1", [passage])?;
            tx.execute("DELETE FROM history_passages WHERE id=?1", [passage])?;
            tx.execute(
                "DELETE FROM history_projection WHERE session=?1 AND version<?2",
                params![id, VERSION],
            )?;
            tx.commit()?;
            return Ok(true);
        }
        let checkpoint:Option<(u64,u64,String,bool)>=self.db.query_row("SELECT frontier,ordinal,checkpoint,sealed FROM history_projection WHERE session=?1 AND version=?2",params![id,VERSION],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let (frontier, mut ordinal, mut projection, finished) =
            if let Some((frontier, ordinal, json, finished)) = checkpoint {
                (
                    frontier,
                    ordinal,
                    serde_json::from_str::<crate::history::Projection>(&json)?,
                    finished,
                )
            } else {
                (0, 0, Default::default(), false)
            };
        if frontier == durable && (!sealed || finished) {
            return Ok(false);
        }
        let end = (frontier + SLICE).min(durable);
        let raw = self.read_raw(&id, frontier, end)?;
        let mut page = crate::history::ReadablePage::default();
        projection.feed(&raw, frontier, 0..end, &mut page);
        let finished = sealed && end == durable;
        if finished {
            projection.preview(end, 0..end, &mut page);
        }
        #[cfg(debug_assertions)]
        let index_fault = self.directory.join("proof-storage-index-commit");
        let tx = self.db.transaction()?;
        for passage in page.passages {
            let text = &page.text[passage.text_start..passage.text_end];
            tx.execute("INSERT INTO history_passages(session,version,ordinal,raw_start,raw_end,text) VALUES(?1,?2,?3,?4,?5,?6)",params![id,VERSION,ordinal,passage.raw_start,passage.raw_end,text])?;
            let passage_id = tx.last_insert_rowid();
            for term in terms(text) {
                tx.execute(
                    "INSERT INTO history_terms VALUES(?1,?2)",
                    params![term, passage_id],
                )?;
            }
            ordinal += 1;
        }
        tx.execute("INSERT INTO history_projection VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(session) DO UPDATE SET version=excluded.version,frontier=excluded.frontier,ordinal=excluded.ordinal,checkpoint=excluded.checkpoint,sealed=excluded.sealed",params![id,VERSION,end,ordinal,serde_json::to_string(&projection)?,finished])?;
        #[cfg(debug_assertions)]
        if std::fs::read_to_string(index_fault).is_ok_and(|value| value.trim() == "error") {
            return Err(io::Error::from_raw_os_error(libc::ENOSPC).into());
        }
        tx.commit()?;
        Ok(true)
    }
    fn prune_index(&mut self, id: &str) -> Result<bool> {
        let passage: Option<i64> = self
            .db
            .query_row(
                "SELECT id FROM history_passages WHERE session=?1 ORDER BY id LIMIT 1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        let tx = self.db.transaction()?;
        if let Some(passage) = passage {
            tx.execute("DELETE FROM history_terms WHERE passage=?1", [passage])?;
            tx.execute("DELETE FROM history_passages WHERE id=?1", [passage])?;
        }
        tx.execute("DELETE FROM history_projection WHERE session=?1", [id])?;
        tx.commit()?;
        Ok(passage.is_some())
    }
}
