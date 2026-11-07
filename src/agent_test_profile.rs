//! Opt-in native test settings; ordinary user launches retain their configuration.
use portable_pty::CommandBuilder;
#[cfg(feature = "native-agent-qualification")]
use serde::Deserialize;
#[cfg(feature = "native-agent-qualification")]
use std::{collections::BTreeMap, sync::OnceLock};

#[cfg(feature = "native-agent-qualification")]
#[derive(Deserialize)]
pub(crate) struct Profile {
    pub id: String,
    codex_args: Vec<String>,
    claude_args: Vec<String>,
    claude_env: BTreeMap<String, String>,
}

#[cfg(feature = "native-agent-qualification")]
fn profile() -> &'static Profile {
    static PROFILE: OnceLock<Profile> = OnceLock::new();
    PROFILE.get_or_init(|| {
        serde_json::from_str(include_str!("../scripts/agent-test-profile.json"))
            .expect("valid embedded agent test profile")
    })
}

#[cfg(feature = "native-agent-qualification")]
pub(crate) fn active() -> Option<String> {
    (std::env::var("RAT_AGENT_TEST_PROFILE").ok().as_deref() == Some(&profile().id))
        .then(|| profile().id.clone())
}

#[cfg(not(feature = "native-agent-qualification"))]
pub(crate) fn active() -> Option<String> {
    None
}

#[cfg(not(feature = "native-agent-qualification"))]
pub(crate) fn configure(_command: &mut CommandBuilder, _claude: bool) {}

#[cfg(feature = "native-agent-qualification")]
pub(crate) fn configure(command: &mut CommandBuilder, claude: bool) {
    let profile = profile();
    command.args(if claude {
        &profile.claude_args
    } else {
        &profile.codex_args
    });
    if claude {
        for (name, value) in &profile.claude_env {
            command.env(name, value);
        }
    }
}

#[cfg(all(test, feature = "native-agent-qualification"))]
mod tests {
    use super::*;

    #[test]
    fn explicit_test_settings_precede_the_first_prompt_on_new_and_resumed_launches() {
        for claude in [false, true] {
            for resume in [false, true] {
                let mut command = CommandBuilder::new(if claude { "claude" } else { "codex" });
                if resume {
                    command.args(if claude {
                        vec!["--resume", "old-session"]
                    } else {
                        vec!["resume", "old-session"]
                    });
                }
                configure(&mut command, claude);
                command.args(["--", "first prompt"]);
                let argv = command
                    .get_argv()
                    .iter()
                    .map(|a| a.to_str().unwrap())
                    .collect::<Vec<_>>();
                let model = argv.iter().position(|a| *a == "--model").unwrap();
                assert!(model < argv.len() - 2);
                assert_eq!(
                    argv[model + 1],
                    if claude {
                        "claude-haiku-4-5-20251001"
                    } else {
                        "gpt-5.1-codex-mini"
                    }
                );
                if !claude {
                    assert!(argv.contains(&"model_reasoning_effort=\"low\""));
                } else {
                    assert_eq!(command.get_env("MAX_THINKING_TOKENS").unwrap(), "0");
                    assert_eq!(
                        command
                            .get_env("CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING")
                            .unwrap(),
                        "1"
                    );
                }
            }
        }
    }
}
