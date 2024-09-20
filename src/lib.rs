pub mod checker;
pub mod config;
pub mod dl;
pub mod parser;
pub mod res;
pub mod rss;
pub mod serve;

use bytes::Bytes;
use color_eyre::eyre::Result;
use std::sync::LazyLock;

use std::path::PathBuf;

use directories::ProjectDirs;
use tracing_error::ErrorLayer;
use tracing_subscriber::{self, layer::SubscriberExt, util::SubscriberInitExt, Layer};

const VIDEO_EXTS: [&str; 2] = ["mp4", "mkv"];

pub static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    let mut client_builder = reqwest::Client::builder();
    if let Some(config) = config::Config::load().unwrap().proxy {
        let proxy = reqwest::Proxy::all(config.scheme.to_string()).unwrap();
        client_builder = client_builder.proxy(proxy);
    }
    client_builder.build().unwrap()
});

pub async fn get_url_bytes(url: &str) -> Result<Bytes> {
    CLIENT
        .get(url)
        .send()
        .await?
        .bytes()
        .await
        .map_err(|e| e.into())
}

pub static PROJECT_NAME: LazyLock<String> =
    LazyLock::new(|| env!("CARGO_CRATE_NAME").to_uppercase().to_string());
pub static DATA_FOLDER: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    std::env::var(format!("{}_DATA", PROJECT_NAME.clone()))
        .ok()
        .map(PathBuf::from)
});
fn project_directory() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", env!("CARGO_PKG_NAME"))
}

pub fn get_data_dir() -> PathBuf {
    let directory = if let Some(s) = DATA_FOLDER.clone() {
        s
    } else if let Some(proj_dirs) = project_directory() {
        proj_dirs.config_dir().to_path_buf()
    } else {
        PathBuf::from(".").join(".data")
    };

    if !directory.exists() {
        std::fs::create_dir_all(directory.clone()).unwrap();
    }

    directory
}

pub static LOG_ENV: LazyLock<String> =
    LazyLock::new(|| format!("{}_LOGLEVEL", PROJECT_NAME.clone()));
pub static LOG_FILE: LazyLock<String> = LazyLock::new(|| format!("{}.log", env!("CARGO_PKG_NAME")));

pub fn initialize_logging_from_crate_name() -> Result<()> {
    initialize_logging(env!("CARGO_CRATE_NAME"))
}

pub fn initialize_logging(target: &str) -> Result<()> {
    // let directory = get_data_dir();
    // std::fs::create_dir_all(directory.clone())?;
    // let log_path = directory.join(LOG_FILE.clone());
    // println!("{}", log_path.to_str().unwrap());
    // let log_file = std::fs::File::create(log_path)?;
    std::env::set_var(
        "RUST_LOG",
        std::env::var("RUST_LOG")
            .or_else(|_| std::env::var(LOG_ENV.clone()))
            .unwrap_or_else(|_| format!("{target}=info")),
    );
    let file_subscriber = tracing_subscriber::fmt::layer()
        // .with_line_number(true)
        // .with_file(true)
        // .with_writer(log_file)
        .with_target(true)
        .with_ansi(false)
        .with_filter(tracing_subscriber::filter::EnvFilter::from_default_env());
    tracing_subscriber::registry()
        .with(file_subscriber)
        .with(ErrorLayer::default())
        .init();
    Ok(())
}
