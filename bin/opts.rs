use crate::cmd::generate;
use clap::{Parser, Subcommand};

const VERSION_MESSAGE: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("VERGEN_GIT_SHA"),
    " ",
    env!("VERGEN_BUILD_TIMESTAMP"),
    ")"
);

/// Build, test, fuzz, debug and deploy Solidity contracts.
#[derive(Parser)]
#[command(
    name = "cannon",
    version = VERSION_MESSAGE,
    after_help = "Find more information in the book: http://book.getfoundry.sh/reference/forge/forge.html",
    next_display_order = None,
)]
pub struct Cannon {
    #[command(subcommand)]
    pub cmd: CannonSubcommand,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum CannonSubcommand {
    /// Generate scaffold files.
    Generate(generate::GenerateArgs),
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cannon::command().debug_assert();
    }
}
