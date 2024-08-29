use std::collections::HashMap;

use axum::{
    extract::Query,
    http::StatusCode,
    routing::{any, get, post},
    Json, Router,
};
use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;
use http::{header, Method};
use muuf::{
    checker::{check_collection, check_mikan, check_res_rule},
    config::{Collection, Config, Link, Mikan, Rule},
    dl::{self, Client},
    initialize_logging_from_crate_name,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    initialize_logging_from_crate_name()?;
    info!("muuf started");
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

async fn serve() {
    let cors = CorsLayer::new()
        .allow_headers(vec![
            header::ACCEPT,
            header::ACCEPT_LANGUAGE,
            header::AUTHORIZATION,
            header::CONTENT_LANGUAGE,
            header::CONTENT_TYPE,
        ])
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::HEAD,
            Method::OPTIONS,
            Method::CONNECT,
            Method::PATCH,
            Method::TRACE,
        ])
        // allow requests from any origin
        .allow_origin(Any);

    // build our application with a single route
    let app = Router::new()
        .route("/", any(|| async { "Hello, World!" }))
        .route("/request-check", post(request_check))
        .route("/mikan", get(find_mikan))
        .route("/add-mikan", post(add_mikan))
        .route("/rm-mikan", post(rm_mikan))
        .route("/collection", get(find_collection))
        .route("/add-collection", post(add_collection))
        .route("/rm-collection", post(rm_collection));

    // run it with hyper on localhost:3000
    let port = 3000;
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    info!("Listening on port:{port}");
    axum::serve(listener, app.layer(cors)).await.unwrap();
}

#[derive(Serialize)]
struct ApiResponse {
    message: String,
}

fn to_resp(status: StatusCode, message: String) -> (StatusCode, Json<ApiResponse>) {
    (status, Json(ApiResponse { message }))
}

async fn find_mikan(
    Query(params): Query<HashMap<String, String>>,
) -> (StatusCode, Json<Vec<Mikan>>) {
    let config = Config::load().unwrap();
    if let Some(id) = params.get("bangumiId") {
        if let Some(m) = config
            .mikan
            .iter()
            .find(|m| m.url.contains(&format!("bangumiId={id}")))
        {
            return (StatusCode::OK, Json(vec![m.clone()]));
        } else {
            return (StatusCode::NOT_FOUND, Json(vec![]));
        }
    }
    (StatusCode::OK, Json(config.mikan.clone()))
}

async fn add_mikan(Json(m): Json<Mikan>) -> (StatusCode, Json<ApiResponse>) {
    let mut config = Config::load().unwrap();
    match config.add_mikan(m).and_then(|()| config.save()) {
        Err(e) => to_resp(StatusCode::BAD_REQUEST, e.to_string()),
        Ok(_) => to_resp(StatusCode::OK, "add success".to_string()),
    }
}

#[derive(Deserialize)]
struct RmMikanForm {
    url: String,
}

async fn rm_mikan(Json(form): Json<RmMikanForm>) -> (StatusCode, Json<ApiResponse>) {
    let mut config = Config::load().unwrap();
    match config.rm_mikan(&form.url).and_then(|()| config.save()) {
        Err(e) => to_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        Ok(_) => to_resp(StatusCode::OK, "rm sucess".to_string()),
    }
}

async fn find_collection(
    Query(params): Query<HashMap<String, String>>,
) -> (StatusCode, Json<Vec<Collection>>) {
    let config = Config::load().unwrap();
    if let Some(url) = params.get("url") {
        if let Some(c) = config.collections.iter().find(|c| &c.torrent_url == url) {
            return (StatusCode::OK, Json(vec![c.clone()]));
        } else {
            return (StatusCode::NOT_FOUND, Json(vec![]));
        }
    }
    (StatusCode::OK, Json(config.collections.clone()))
}

async fn add_collection(Json(c): Json<Collection>) -> (StatusCode, Json<ApiResponse>) {
    let mut config = Config::load().unwrap();
    match config.add_collection(c).and_then(|()| config.save()) {
        Err(e) => to_resp(StatusCode::BAD_REQUEST, e.to_string()),
        Ok(_) => to_resp(StatusCode::OK, "add success".to_string()),
    }
}

#[derive(Deserialize)]
struct RmCollectionForm {
    url: String,
}

async fn rm_collection(Json(form): Json<RmCollectionForm>) -> (StatusCode, Json<ApiResponse>) {
    let mut config = Config::load().unwrap();
    match config.rm_collection(&form.url).and_then(|()| config.save()) {
        Err(e) => to_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        Ok(_) => to_resp(StatusCode::OK, "rm sucess".to_string()),
    }
}

async fn request_check() -> (StatusCode, Json<ApiResponse>) {
    tokio::spawn(async move { check().await });
    to_resp(StatusCode::OK, "check requested".to_string())
}

async fn check() -> Result<()> {
    let config = Config::load()?;
    let result = check_with_config(&config).await;
    if let Err(e) = &result {
        error!("{:?}", e);
    }
    result
}

async fn check_with_config(config: &Config) -> Result<()> {
    let mut dl_client = dl::get_client(&config.downloader);
    let dl_server_torrents = dl_client.torrent_get().await?;
    let mut added_torrent_hashs = Vec::new();

    check_res_rules(
        &mut dl_client,
        &dl_server_torrents,
        &mut added_torrent_hashs,
        &config.rules,
    )
    .await?;

    check_mikan_rss(
        &mut dl_client,
        &dl_server_torrents,
        &mut added_torrent_hashs,
        &config.mikan,
        &config.link,
    )
    .await?;

    check_collections(
        &mut dl_client,
        &dl_server_torrents,
        &mut added_torrent_hashs,
        &config.collections,
        &config.link,
    )
    .await?;

    Ok(())
}

async fn check_res_rules(
    dl_client: &mut dyn Client,
    dl_server_torrents: &[dl::Torrent],
    added_torrent_hashs: &mut Vec<String>,
    rules: &[Rule],
) -> Result<()> {
    info!("{} rules to be checked", rules.len());
    for rule in rules.iter() {
        check_res_rule(rule, dl_client, dl_server_torrents, added_torrent_hashs).await?;
    }
    Ok(())
}

async fn check_mikan_rss(
    dl_client: &mut dyn Client,
    dl_server_torrents: &[dl::Torrent],
    added_torrent_hashs: &mut Vec<String>,
    mikan: &[Mikan],
    maybe_link: &Option<Link>,
) -> Result<()> {
    info!("{} mikan rss to be checked", mikan.len());
    for m in mikan {
        check_mikan(
            m,
            dl_client,
            dl_server_torrents,
            added_torrent_hashs,
            maybe_link,
        )
        .await?;
    }

    if !mikan.is_empty() {
        info!("done checking mikan")
    }

    Ok(())
}

async fn check_collections(
    dl_client: &mut dyn Client,
    dl_server_torrents: &[dl::Torrent],
    added_torrent_hashs: &mut Vec<String>,
    collections: &[Collection],
    maybe_link: &Option<Link>,
) -> Result<()> {
    info!("{} collection to be checked", collections.len());
    for collection in collections {
        check_collection(
            collection,
            dl_client,
            dl_server_torrents,
            added_torrent_hashs,
            maybe_link,
        )
        .await?;
    }

    if !collections.is_empty() {
        info!("done checking collections")
    }

    Ok(())
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
