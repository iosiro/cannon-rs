use clap::Parser;
use cmd::router::RouterSubcommands;
use eyre::Result;
use foundry_cli::{handler, utils};

mod cmd;

mod opts;
use opts::{Cannon, CannonSubCommand};

#[tokio::main]
async fn main() -> Result<()> {
    handler::install();
    utils::load_dotenv();
    utils::subscriber();
    utils::enable_paint();

    let opts = Cannon::parse();
    match opts.cmd {
        CannonSubCommand::Router(cmd) => match cmd.sub {
            RouterSubcommands::Generate(cmd) => cmd.run(),
        },
    }
}
