use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;
use muuf::{checker::check, config::Config, initialize_logging_from_crate_name, serve::serve};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    initialize_logging_from_crate_name()?;
    info!("muuf started, version: {}", env!("CARGO_PKG_VERSION"));
    let cli = Cli::parse();
    match cli.commands {
        Commands::Watch => watch().await,
        Commands::Serve => serve().await,
        Commands::Check => check().await?,
        Commands::Validate => validate(),
    }

    Ok(())
}

fn validate() {
    let _ = Config::load().unwrap();
    info!("Config is valid");
}

async fn watch() {
    let config = Config::load().unwrap();
    tokio::spawn(async move {
        let interval = config.check_interval;
        loop {
            let _ = check().await;
            tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
        }
    });
    serve().await;
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 持续运行，间隔{interval}时间检查
    Watch,
    /// 仅提供API
    Serve,
    /// 检查一次
    Check,
    /// 校验
    Validate,
}
