use crate::cmd::router;
use clap::{Parser, Subcommand};

const VERSION_MESSAGE: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("VERGEN_GIT_SHA"),
    " ",
    env!("VERGEN_BUILD_TIMESTAMP"),
    ")"
);

/// Interact with Cannon files
#[derive(Parser)]
#[command(
    name = "cannon",
    version = VERSION_MESSAGE,
    next_display_order = None,
)]
pub struct Cannon {
    #[command(subcommand)]
    pub cmd: CannonSubCommand,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum CannonSubCommand {
    /// Router commands
    Router(router::RouterArgs),
}
