use bytes::Bytes;
use color_eyre::eyre::Result;
use serde::Deserialize;

use crate::{get_url_bytes, CLIENT};

mod mikan;

#[derive(Debug, Deserialize)]
struct MikanRssContainer {
    channel: MikanRss,
}

#[derive(Debug, Deserialize)]
struct MikanRss {
    #[serde(rename = "item")]
    items: Vec<MikanRssItem>,
}

#[derive(Debug, Deserialize)]
struct MikanRssItem {
    title: String,
    //     torrent: MikanRssItemTorrent,
    enclosure: MikanRssItemEnclosure,
}

#[derive(Debug, Deserialize)]
struct MikanRssItemEnclosure {
    #[serde(rename = "@url")]
    url: String,
}

pub async fn parse_mikan(url: &str) -> Result<Vec<(String, String, Result<Bytes>)>> {
    let rss_text = CLIENT.get(url).send().await?.text().await?;
    let r = quick_xml::de::from_str::<MikanRssContainer>(&rss_text)?;

    let x = r.channel.items.iter().map(|item| async {
        (
            item.title.to_string(),
            item.enclosure.url.to_string(),
            get_url_bytes(&item.enclosure.url).await,
        )
    });

    Ok(futures::future::join_all(x).await)
}
