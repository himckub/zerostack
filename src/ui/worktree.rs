use crossterm::event::KeyCode;
use tokio::sync::mpsc;

use crate::cli::Cli;
use crate::config::Config;
use crate::context::ContextFiles;
use crate::event::UserEvent;
#[cfg(feature = "mcp")]
use crate::extras::mcp::McpClientManager;
use crate::permission::ask::AskSender;
use crate::permission::checker::PermCheck;
use crate::provider::{AnyAgent, AnyClient};
use crate::sandbox::Sandbox;
use crate::session::Session;

use super::dot_cmd::apply_current_prompt_mode;
use super::events::render_session;
use super::renderer::Renderer;
use super::{C_AGENT, C_ERROR, C_PERM};

/// Set up a git worktree, change directory into it, rebuild the agent, and re-render.
pub async fn setup_worktree_env(
    name: &str,
    session: &mut Session,
    context: &mut ContextFiles,
    client: &AnyClient,
    agent: &mut Option<AnyAgent>,
    renderer: &mut Renderer,
    cli: &Cli,
    cfg: &Config,
    permission: &Option<PermCheck>,
    ask_tx: &Option<AskSender>,
    sandbox: &Sandbox,
    reasoning_enabled: bool,
    #[cfg(feature = "mcp")] mcp_manager: Option<&McpClientManager>,
) {
    let wt_base_dir = cli.resolve_wt_base_dir(cfg);
    match crate::extras::git_worktree::create(name, wt_base_dir.as_deref()) {
        Ok((path, _info)) => {
            std::env::set_current_dir(&path).ok();
            session.working_dir = compact_str::CompactString::new(path.to_string_lossy());
            context.reload();
            apply_current_prompt_mode(context, permission);
            let model = client.completion_model(session.model.to_string());
            *agent = Some(
                crate::provider::build_agent(
                    model,
                    cli,
                    cfg,
                    context,
                    permission.clone(),
                    ask_tx.clone(),
                    sandbox.clone(),
                    reasoning_enabled,
                    #[cfg(feature = "mcp")]
                    mcp_manager,
                )
                .await,
            );
            let _ = render_session(renderer, session, cli, cfg, context);
        }
        Err(e) => {
            let _ = renderer.write_line(&format!("worktree failed: {}", e), C_ERROR);
        }
    }
}

/// Auto-merge a worktree when the session exits (called if `--wt-auto-merge` is set).
#[cfg(feature = "git-worktree")]
pub async fn handle_auto_merge(
    renderer: &mut Renderer,
    user_rx: &mut mpsc::Receiver<UserEvent>,
    cli: &Cli,
    cfg: &Config,
) {
    if !cli.resolve_wt_auto_merge(cfg) {
        return;
    }
    let Some(info) = crate::extras::git_worktree::detect() else {
        return;
    };
    let target = crate::extras::git_worktree::default_branch(&info.main_repo_path)
        .unwrap_or_else(|| "main".to_string());

    let _ = renderer.write_line(
        &format!(
            "auto-merging worktree '{}' into '{}'...",
            info.branch, target
        ),
        C_AGENT,
    );
    let (state, outcome) = crate::extras::git_worktree::try_merge(&info, &target);
    match outcome {
        crate::extras::git_worktree::MergeOutcome::Success => {
            match crate::extras::git_worktree::complete_merge(&state) {
                Ok(()) => {
                    let _ = renderer.write_line(
                        &format!("merged '{}' into '{}' and cleaned up", info.branch, target),
                        C_AGENT,
                    );
                }
                Err(e) => {
                    let _ = renderer.write_line(
                        &format!("merge succeeded but cleanup failed: {}", e),
                        C_ERROR,
                    );
                }
            }
        }
        crate::extras::git_worktree::MergeOutcome::Conflicts(files) => {
            let _ = renderer.write_line(
                &format!("merge conflict in {} file(s):", files.len()),
                C_ERROR,
            );
            for f in &files {
                let _ = renderer.write_line(&format!("  {}", f), C_ERROR);
            }
            let _ =
                renderer.write_line("Keep conflict state for manual resolution? [y/N] ", C_PERM);

            let abort = loop {
                tokio::select! {
                    Some(ev) = user_rx.recv() => {
                        if let UserEvent::Key(key) = ev {
                            match key.code {
                                KeyCode::Char('y') | KeyCode::Char('Y') => break false,
                                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc | KeyCode::Enter => break true,
                                _ => {}
                            }
                        }
                    }
                }
            };

            if abort {
                let _ = crate::extras::git_worktree::cancel_merge(&state);
                let _ = renderer.write_line("merge aborted, restored original state", C_AGENT);
            } else {
                let _ = renderer.write_line(
                    &format!(
                        "conflict state left in {} for manual resolution",
                        info.main_repo_path.display()
                    ),
                    C_AGENT,
                );
            }
        }
        crate::extras::git_worktree::MergeOutcome::Error(e) => {
            let _ = renderer.write_line(&format!("merge failed: {}", e), C_ERROR);
        }
    }
}
