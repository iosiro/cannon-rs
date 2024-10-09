use clap::Parser;
use eyre::Result;
use foundry_cli::{handler, utils};

mod cmd;
use cmd::generate::GenerateSubcommands;

mod opts;
use opts::{Cannon, CannonSubcommand};

fn main() -> Result<()> {
    handler::install();
    utils::load_dotenv();
    utils::subscriber();
    utils::enable_paint();

    let opts = Cannon::parse();

    match opts.cmd {
        CannonSubcommand::Generate(cmd) => match cmd.sub {
            GenerateSubcommands::Router(cmd) => cmd.run(),
            GenerateSubcommands::DynamicRouter(cmd) => cmd.run(),
        },
    }
}
