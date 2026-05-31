mod cmd_picker;
pub(crate) mod dot_cmd;
mod event_handler;
mod events;
pub(crate) mod input;
pub(crate) mod interactive;
mod markdown;
mod permission_handler;
pub(crate) mod picker;
pub(crate) mod renderer;
mod slash;
mod status;
mod terminal;
pub(crate) mod utils;
mod worktree;

pub(crate) use dot_cmd::apply_current_prompt_mode;

use crossterm::style::Color;

use crate::cli::Cli;
use crate::config::Config;
use crate::context::ContextFiles;
use crate::permission::ask::{AskReceiver, AskSender};
use crate::permission::checker::PermCheck;
use crate::provider::{AnyAgent, AnyClient};
use crate::sandbox::Sandbox;
use crate::session::Session;
use crate::ui::interactive::InteractiveSession;

pub(super) const C_AGENT: Color = Color::White;
pub(super) const C_ERROR: Color = Color::Red;
pub(super) const C_TOOL: Color = Color::Yellow;
pub(super) const C_PERM: Color = Color::Magenta;

#[allow(clippy::too_many_arguments)]
pub async fn run_interactive(
    client: AnyClient,
    agent: Option<AnyAgent>,
    cli: &Cli,
    cfg: &Config,
    session: &mut Session,
    context: &mut ContextFiles,
    permission: Option<PermCheck>,
    ask_tx: Option<AskSender>,
    ask_rx: Option<AskReceiver>,
    sandbox: Sandbox,
) -> anyhow::Result<()> {
    let mut is: InteractiveSession<'_> = InteractiveSession::new(
        client, agent, cli, cfg, session, context, permission, ask_tx, ask_rx, sandbox,
    )
    .await?;
    is.run().await
}
