use immutable::GenerateImmutableRouterArgs;
use clap::{Parser, Subcommand};
use create2::GenerateRouterArgs;


mod immutable;
mod create2;

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
    ImmutableRouter(Box<GenerateImmutableRouterArgs>),
}
