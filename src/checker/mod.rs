const VIDEO_EXTS: [&str; 2] = ["mp4", "mkv"];

mod collection;
mod mikan;
mod res_rule;

pub use collection::check_collection;
pub use mikan::check_mikan;
pub use res_rule::check_res_rule;

use crate::{
    config::{Collection, Config, Link, Mikan, Rule},
    dl::{self, Client},
};
use color_eyre::eyre::Result;
use tracing::{error, info};

pub async fn check() -> Result<()> {
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
