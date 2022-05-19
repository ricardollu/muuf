use anyhow::Result;
use async_trait::async_trait;
use transmission_rpc::{
    types::{BasicAuth, RpcResponse, TorrentAddArgs, TorrentAdded, TorrentGetField, Torrents},
    TransClient,
};

use crate::config::Downloader;

pub fn get_client(downloader_config: &Downloader) -> impl Client {
    match downloader_config {
        Downloader::Transmission(config) => Transmission {
            client: TransClient::with_auth(
                &config.url,
                BasicAuth {
                    user: config.user.to_string(),
                    password: config.password.to_string(),
                },
            ),
        },
    }
}

#[async_trait]
pub trait Client {
    async fn torrent_add(&self, magnet: String, folder: &str) -> Result<()>;
    async fn torrent_get(&self) -> Result<Vec<Torrent>>;
}

pub struct Torrent {
    pub hash: String,
}

struct Transmission {
    client: TransClient,
}

#[async_trait]
impl Client for Transmission {
    async fn torrent_add(&self, magnet: String, folder: &str) -> Result<()> {
        let add: TorrentAddArgs = TorrentAddArgs {
            filename: Some(magnet),
            download_dir: Some(format!("/downloads/muuf/{}/", folder)),
            ..TorrentAddArgs::default()
        };
        let resp: RpcResponse<TorrentAdded> = self.client.torrent_add(add).await.map_err(|e| anyhow::anyhow!(e))?;
        if resp.is_ok() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Error adding torrent"))
        }
    }

    async fn torrent_get(&self) -> Result<Vec<Torrent>> {
        let resp: RpcResponse<Torrents<transmission_rpc::types::Torrent>> = self
            .client
            .torrent_get(Some(vec![TorrentGetField::Id, TorrentGetField::HashString, TorrentGetField::Name]), None)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let torrents: Vec<Torrent> = resp
            .arguments
            .torrents
            .into_iter()
            .map(|it| Torrent {
                hash: it.hash_string.unwrap_or_else(|| String::from("")),
            })
            .collect();

        Ok(torrents)
    }
}
