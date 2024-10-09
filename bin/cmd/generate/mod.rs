use clap::{Parser, Subcommand};

use crate::cmd::generate::dynamic::GenerateDynamicRouterArgs;
use crate::cmd::generate::router::GenerateRouterArgs;

pub mod dynamic;
pub mod router;

/// CLI arguments for `forge generate`.
#[derive(Debug, Parser)]
pub struct GenerateArgs {
    #[command(subcommand)]
    pub sub: GenerateSubcommands,
}

#[derive(Debug, Subcommand)]
pub enum GenerateSubcommands {
    /// Generate ERCXXX router.
    Router(Box<GenerateRouterArgs>),
    DynamicRouter(Box<GenerateDynamicRouterArgs>),
}
