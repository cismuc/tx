use clap::Parser;
use tx::cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cmd = cli.resolved_command();
    println!("tx starting with command: {:?}", cmd);
    Ok(())
}
