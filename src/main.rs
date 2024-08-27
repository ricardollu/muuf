use std::{
    collections::HashMap,
    fs,
    path::{self, PathBuf},
};

use axum::{
    extract::Query,
    http::StatusCode,
    routing::{any, get, post},
    Json, Router,
};
use base64::{engine::general_purpose, Engine};
use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;
use config::Config;
use http::{header, Method};
use muuf::{
    config::{Collection, Matcher, Mikan, SeasonFolder, SpecialMapping},
    dl::Client,
    rss::parse_mikan,
    *,
};
use res::ApiServer;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

const VIDEO_EXTS: [&str; 2] = ["mp4", "mkv"];

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

async fn check() -> Result<()> {
    let config = Config::load()?;
    check_with_config(&config).await
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
    rules: &[config::Rule],
) -> Result<()> {
    println!("{} rules to be checked", rules.len());
    for rule in rules.iter() {
        let res_api = res::get_res_api(&rule.res_api);
        let (res_list, _) = res_api
            .res_list(
                &rule.keywords,
                rule.sub_group_id,
                rule.res_type_id,
                rule.publish_after,
            )
            .await?;
        for res in res_list {
            if dl_server_torrents.iter().any(|t| res.info_hash == t.hash)
                || added_torrent_hashs.contains(&res.info_hash)
            {
                // println!("{} already in download server", res.title);
                continue;
            }
            dl_client
                .torrent_add(res.magnet.to_string(), &rule.name)
                .await?;
            added_torrent_hashs.push(res.info_hash);
            println!("加入下载列表: {}", res.title)
        }
    }
    Ok(())
}

async fn check_mikan_rss(
    dl_client: &mut dyn Client,
    dl_server_torrents: &[dl::Torrent],
    added_torrent_hashs: &mut Vec<String>,
    mikan: &[Mikan],
    maybe_link: &Option<config::Link>,
) -> Result<()> {
    println!("{} mikan rss to be checked", mikan.len());
    for m in mikan {
        let mut ts = parse_mikan(&m.url).await?;
        for e in &m.extra {
            ts.push((e.title.clone(), e.url.clone(), get_url_bytes(&e.url).await));
        }
        let ts = ts
            .into_iter()
            .filter(|(_, _, maybe_bytes)| maybe_bytes.is_ok())
            .map(|(a, b, c)| (a, b, c.unwrap()));

        for (title, url, bytes) in ts {
            if m.skip.iter().any(|s| {
                if s.title.trim() == "" {
                    url.trim() == s.url.trim()
                } else if s.url.trim() == "" {
                    title.trim() == s.title.trim()
                } else {
                    title.trim() == s.title.trim() && url.trim() == s.url.trim()
                }
            }) {
                continue;
            }
            if !m.title_contain.iter().all(|s| title.contains(s)) {
                continue;
            }

            if title.contains("合集") {
                // println!("跳过合集: {} ", title);
                continue;
            }

            let torrent = lava_torrent::torrent::v1::Torrent::read_from_bytes(&bytes)?;
            let pathbuf_torrent_name = PathBuf::from(&torrent.name);
            // If the torrent contains only 1 file then files is None.
            let (file_name_from_torrent, file_stem, storage_path) = if torrent.files.is_some() {
                let mut some_file_name_from_torrent = None;
                for file in torrent.files.as_ref().unwrap() {
                    let file_suffix = file.path.extension().unwrap().to_str().unwrap();
                    if VIDEO_EXTS.iter().any(|ext| ext == &file_suffix) {
                        some_file_name_from_torrent = Some((
                            file.path.file_name().unwrap().to_str().unwrap(),
                            file.path.file_stem().unwrap().to_str().unwrap(),
                            format!("{}/", &torrent.name),
                        ));
                        break;
                    }
                }
                if let Some(n) = some_file_name_from_torrent {
                    n
                } else {
                    println!("跳过没有视频文件的多文件种子: {}", title);
                    continue;
                }
            } else {
                (
                    torrent.name.as_str(),
                    pathbuf_torrent_name.file_stem().unwrap().to_str().unwrap(),
                    "".to_string(),
                )
            };

            let some_server_torrent = dl_server_torrents
                .iter()
                .find(|t| t.hash == torrent.info_hash());
            if let Some(server_torrent) = some_server_torrent {
                if let Some(link_config) = maybe_link {
                    if link_config.enable
                        && (server_torrent.percent_done >= 1.0 || link_config.dry_run)
                    {
                        // If the torrent contains only 1 file then name is the file name. Otherwise it’s the suggested root directory’s name.
                        // let file_name_from_torrent = &torrent.name;
                        let file_suffix = file_name_from_torrent.split('.').last().unwrap();
                        let ep = parser::process(&title)?;
                        let name = ep.name(Some(&m.name))?;
                        let path = ep.link_path(&name);
                        let link_file_name = ep.link_file_name(&name);

                        let full_path = format!("{}/{path}", &link_config.path);
                        let full_file_name = format!("{link_file_name}.{file_suffix}");
                        let link = format!("{full_path}/{full_file_name}");
                        if !path::Path::new(&link).exists() {
                            if link_config.dry_run {
                                println!(
                                    "准备链接{link} <- {storage_path}{file_name_from_torrent}"
                                );
                            } else {
                                fs::create_dir_all(&full_path)?;
                                let original = format!(
                                    "{}/{storage_path}{file_name_from_torrent}",
                                    &server_torrent.download_dir,
                                );
                                match fs::hard_link(&original, &link) {
                                    Ok(_) => {
                                        println!("创建链接{link} <- {storage_path}{file_name_from_torrent}")
                                    }
                                    Err(e) => println!("硬链接失败: {} 当{link} <- {original}", e),
                                }
                            }
                        }

                        // 外挂字幕
                        if m.external_subtitle {
                            muuf::parser::link_external_subtitle(
                                &torrent,
                                file_stem,
                                &full_path,
                                &link_file_name,
                                link_config,
                                server_torrent,
                            )?;
                        }
                    }
                }

                continue;
            }
            if added_torrent_hashs.contains(&torrent.info_hash()) {
                println!("{} 刚刚已经被加入下载了", title);
                continue;
            }
            dl_client
                .torrent_add_by_meta(general_purpose::STANDARD.encode(bytes), &m.name)
                .await?;
            added_torrent_hashs.push(torrent.info_hash());
            println!("加入下载列表: {}", title)
        }
    }

    Ok(())
}

async fn check_collections(
    dl_client: &mut dyn Client,
    dl_server_torrents: &[dl::Torrent],
    added_torrent_hashs: &mut Vec<String>,
    collections: &[Collection],
    maybe_link: &Option<config::Link>,
) -> Result<()> {
    println!("{} collection to be checked", collections.len());
    for Collection {
        name,
        torrent_url,
        title,
        season_folders,
        special_mappings,
        external_subtitle,
    } in collections
    {
        let bytes = get_url_bytes(torrent_url).await?;
        let torrent = lava_torrent::torrent::v1::Torrent::read_from_bytes(&bytes)?;
        // If the torrent contains only 1 file then files is None.
        if torrent.files.is_none() {
            println!("不是多文件种子: {}", title);
            continue;
        }

        let some_server_torrent = dl_server_torrents
            .iter()
            .find(|t| t.hash == torrent.info_hash());
        if let Some(server_torrent) = some_server_torrent {
            if let Some(link_config) = maybe_link {
                if link_config.enable && server_torrent.percent_done >= 1.0 {
                    for file in torrent.files.as_ref().unwrap() {
                        let file_name_from_torrent =
                            file.path.file_name().unwrap().to_str().unwrap();
                        let file_suffix = file.path.extension().unwrap().to_str().unwrap();
                        if VIDEO_EXTS.iter().all(|ext| ext != &file_suffix) {
                            continue;
                        }
                        let file_stem = file.path.file_stem().unwrap().to_str().unwrap();

                        let season;
                        let link_file_name;

                        if let Some(SpecialMapping { name, matcher, .. }) =
                            special_mappings.iter().find(|sm| match &sm.matcher {
                                Matcher::Off => sm.file_name == file_name_from_torrent,
                                Matcher::On(regex) => regex.is_match(file_name_from_torrent),
                            })
                        {
                            season = &0_u8;
                            match matcher {
                                Matcher::Off => link_file_name = name.to_string(),
                                Matcher::On(regex) => {
                                    let captures = regex.captures(file_name_from_torrent).unwrap();
                                    let mut name = name.to_string();
                                    for (i, cap) in captures.iter().enumerate() {
                                        if i == 0 {
                                            continue;
                                        }
                                        name =
                                            name.replace(&format!("{{{i}}}"), cap.unwrap().as_str())
                                    }
                                    link_file_name = name;
                                }
                            }
                        } else {
                            let parent = file.path.parent().unwrap().to_str().unwrap();
                            let maybe_season = if let Some(SeasonFolder { season, .. }) =
                                season_folders.iter().find(|sf| sf.folder == parent)
                            {
                                Some(season)
                            } else {
                                None
                            };
                            if maybe_season.is_none() {
                                continue;
                            }
                            season = maybe_season.unwrap();
                            let maybe_real_ep = parser::process(file_name_from_torrent);
                            if maybe_real_ep.is_err() {
                                println!(
                                    "{file_name_from_torrent} 解析失败: {}",
                                    maybe_real_ep.err().unwrap()
                                );
                                continue;
                            }
                            let ep = maybe_real_ep.unwrap().episode;
                            link_file_name = parser::link_file_name(name, season, &ep);
                        }

                        let path = parser::link_path(name, season);
                        let full_path = format!("{}/{path}", &link_config.path);
                        let full_file_name = format!("{}.{file_suffix}", link_file_name);
                        let link = format!("{full_path}/{full_file_name}");
                        if !path::Path::new(&link).exists() {
                            let original = format!(
                                "{}/{}/{}",
                                &server_torrent.download_dir,
                                &torrent.name,
                                file.path.to_str().unwrap()
                            );
                            if link_config.dry_run {
                                println!("准备链接{link} <- {original}",);
                            } else {
                                fs::create_dir_all(&full_path)?;
                                match fs::hard_link(&original, &link) {
                                    Ok(_) => {
                                        println!(
                                            "创建链接{link} <- {}/{file_name_from_torrent}",
                                            &torrent.name
                                        );
                                    }
                                    Err(e) => println!("硬链接失败: {} 当{link} <- {original}", e),
                                }
                            }
                        }

                        // 外挂字幕
                        if *external_subtitle {
                            muuf::parser::link_external_subtitle(
                                &torrent,
                                file_stem,
                                &full_path,
                                &link_file_name,
                                link_config,
                                server_torrent,
                            )?
                        }
                    }
                }
            }

            continue;
        }
        if added_torrent_hashs.contains(&torrent.info_hash()) {
            println!("{} 刚刚已经被加入下载了", title);
            continue;
        }
        dl_client
            .torrent_add_by_meta(general_purpose::STANDARD.encode(bytes), name)
            .await?;
        added_torrent_hashs.push(torrent.info_hash());
        println!("加入下载列表: {}", title)
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
