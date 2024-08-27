use async_trait::async_trait;
use color_eyre::eyre::{eyre, Result};
use reqwest::Url;
use transmission_rpc::{
    types::{
        BasicAuth, Id, RpcResponse, TorrentAddArgs, TorrentAddedOrDuplicate, TorrentGetField,
        TorrentSetArgs, Torrents, TrackerList,
    },
    TransClient,
};

use crate::config::Downloader;

pub fn get_client(downloader_config: &Downloader) -> impl Client {
    match downloader_config {
        Downloader::Transmission(config) => Transmission {
            client: TransClient::with_auth(
                Url::parse(&config.url).unwrap(),
                BasicAuth {
                    user: config.user.to_string(),
                    password: config.password.to_string(),
                },
            ),
        },
    }
}

#[async_trait]
pub trait Client: Send {
    async fn torrent_add(&mut self, magnet: String, folder: &str) -> Result<()>;
    async fn torrent_add_by_meta(&mut self, meta: String, folder: &str) -> Result<()>;
    async fn torrent_set_tracker_list(
        &mut self,
        torrents: &[&Torrent],
        tracker_list: Vec<String>,
    ) -> Result<()>;
    async fn torrent_get(&mut self) -> Result<Vec<Torrent>>;
}

#[derive(Debug)]
pub struct Torrent {
    pub id: i64,
    pub hash: String,
    pub name: String,
    pub download_dir: String,
    pub percent_done: f32,
    pub torrent_file: String,
    pub trackers: Vec<String>,
}

struct Transmission {
    client: TransClient,
}

#[async_trait]
impl Client for Transmission {
    async fn torrent_add(&mut self, magnet: String, folder: &str) -> Result<()> {
        let add: TorrentAddArgs = TorrentAddArgs {
            filename: Some(magnet),
            download_dir: Some(format!("/downloads/muuf/{}/", folder)),
            ..TorrentAddArgs::default()
        };
        let resp: RpcResponse<TorrentAddedOrDuplicate> =
            self.client.torrent_add(add).await.map_err(|e| eyre!(e))?;
        if resp.is_ok() {
            Ok(())
        } else {
            Err(eyre!("Error adding torrent"))
        }
    }

    async fn torrent_add_by_meta(&mut self, meta: String, folder: &str) -> Result<()> {
        let add: TorrentAddArgs = TorrentAddArgs {
            metainfo: Some(meta),
            download_dir: Some(format!("/downloads/muuf/{}/", folder)),
            paused: Some(false),
            ..TorrentAddArgs::default()
        };
        let resp: RpcResponse<TorrentAddedOrDuplicate> =
            self.client.torrent_add(add).await.map_err(|e| eyre!(e))?;
        if resp.is_ok() {
            Ok(())
        } else {
            Err(eyre!("Error adding torrent"))
        }
    }

    async fn torrent_set_tracker_list(
        &mut self,
        torrents: &[&Torrent],
        tracker_list: Vec<String>,
    ) -> Result<()> {
        self.client
            .torrent_set(
                TorrentSetArgs {
                    tracker_list: Some(TrackerList(tracker_list)),
                    ..TorrentSetArgs::default()
                },
                Some(torrents.iter().map(|t| Id::Id(t.id)).collect()),
            )
            .await
            .map_err(|e| eyre!(e))?;
        Ok(())
    }

    async fn torrent_get(&mut self) -> Result<Vec<Torrent>> {
        let resp: RpcResponse<Torrents<transmission_rpc::types::Torrent>> = self
            .client
            .torrent_get(
                Some(vec![
                    TorrentGetField::Id,
                    TorrentGetField::HashString,
                    TorrentGetField::Name,
                    TorrentGetField::DownloadDir,
                    TorrentGetField::PercentDone,
                    TorrentGetField::TorrentFile,
                    TorrentGetField::Trackers,
                ]),
                None,
            )
            .await
            .map_err(|e| eyre!(e))?;
        let torrents: Vec<Torrent> = resp
            .arguments
            .torrents
            .into_iter()
            .map(|it| Torrent {
                id: it.id.unwrap(),
                hash: it.hash_string.unwrap(),
                name: it.name.unwrap(),
                download_dir: it.download_dir.unwrap(),
                percent_done: it.percent_done.unwrap(),
                torrent_file: it.torrent_file.unwrap(),
                trackers: it
                    .trackers
                    .unwrap()
                    .into_iter()
                    .map(|x| x.announce)
                    .collect(),
            })
            .collect();

        Ok(torrents)
    }
}
