use chrono::NaiveDateTime;
use color_eyre::eyre::{eyre, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::get_data_dir;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub mikan: Vec<Mikan>,
    pub downloader: Downloader,
    pub res_api: ResApi,
    pub proxy: Option<Proxy>,
    pub check_interval: u64,
    pub link: Option<Link>,
    #[serde(default)]
    pub collections: Vec<Collection>,
}

impl PartialEq for Config {
    fn eq(&self, other: &Self) -> bool {
        self.rules == other.rules
            && self.mikan == other.mikan
            && self.downloader == other.downloader
            && self.res_api == other.res_api
            && self.proxy == other.proxy
            && self.check_interval == other.check_interval
            && self.link == other.link
            && self.collections == other.collections
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Hash, Eq)]
#[serde(rename_all(deserialize = "lowercase", serialize = "lowercase"))]
pub enum ResApi {
    Dmhy,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Proxy {
    pub scheme: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all(deserialize = "lowercase", serialize = "lowercase")
)]
pub enum Downloader {
    Transmission(TransmissionConfig),
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct TransmissionConfig {
    pub url: String,
    pub user: String,
    pub password: String,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Rule {
    pub name: String,
    pub keywords: Vec<String>,
    pub res_api: ResApi,
    pub sub_group_id: Option<i32>,
    pub sub_group_name: Option<String>,
    pub res_type_id: Option<i32>,
    pub res_type_name: Option<String>,
    pub publish_after: Option<NaiveDateTime>,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct Mikan {
    pub url: String,
    pub name: String,
    #[serde(default)]
    pub extra: Vec<MikanItem>,
    #[serde(default)]
    pub skip: Vec<MikanItem>,
    #[serde(default)]
    pub title_contain: Vec<String>,
    #[serde(default)]
    pub external_subtitle: bool,
    #[serde(default)]
    pub ep_revise: i8,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct MikanItem {
    pub title: String,
    pub url: String,
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
pub struct Collection {
    pub torrent_url: String,
    pub name: String,
    pub title: String,
    #[serde(default)]
    pub season_folders: Vec<SeasonFolder>,
    #[serde(default)]
    pub special_mappings: Vec<SpecialMapping>,
    #[serde(default)]
    pub external_subtitle: bool,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct SeasonFolder {
    pub season: u8,
    pub folder: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SpecialMapping {
    pub file_name: String,
    pub name: String,
    #[serde(default)]
    pub match_and_replace: bool,
    #[serde(skip_deserializing, skip_serializing)]
    pub matcher: Matcher,
}

#[derive(Debug, Clone, Default)]
pub enum Matcher {
    On(Regex),
    #[default]
    Off,
}

impl PartialEq for SpecialMapping {
    fn eq(&self, other: &Self) -> bool {
        self.file_name == other.file_name
            && self.name == other.name
            && self.match_and_replace == other.match_and_replace
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Link {
    pub enable: bool,
    pub path: String,
    #[serde(default)]
    pub dry_run: bool,
    pub notify: Option<LinkNotify>,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum LinkNotify {
    Ntfy { channel: String },
}

const CONFIG_FILE_NAME: &str = "muuf.toml";
// const MIKAN_CONFIG_FILE_NAME: &str = "mikan.toml";

impl Config {
    pub fn load() -> Result<Config> {
        let data_dir = get_data_dir();
        let config_str = std::fs::read_to_string(data_dir.join(CONFIG_FILE_NAME))?;
        let mut config: Config = toml::from_str(&config_str)?;

        // compile regex in collection
        for collection in config.collections.iter_mut() {
            for mapping in collection.special_mappings.iter_mut() {
                if mapping.match_and_replace {
                    mapping.matcher = Matcher::On(Regex::new(&mapping.file_name)?);
                }
            }
        }

        // load standalone mikan configs
        // let mikan_configs: Vec<Mikan> = toml::from_str(&std::fs::read_to_string(
        //     data_dir.join(MIKAN_CONFIG_FILE_NAME),
        // )?)?;
        // config.mikan.extend(mikan_configs);

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        std::fs::write(
            get_data_dir().join(CONFIG_FILE_NAME),
            toml::to_string(self)?,
        )?;
        Ok(())
    }

    pub fn add_mikan(&mut self, mikan: Mikan) -> Result<()> {
        let maybe_pos = self.mikan.iter().position(|m| m.url == mikan.url);
        if let Some(pos) = maybe_pos {
            self.mikan[pos] = mikan;
            Ok(())
        } else {
            self.mikan.push(mikan);
            Ok(())
        }
    }

    pub fn rm_mikan(&mut self, url: &str) -> Result<()> {
        let index = self.mikan.iter().position(|m| m.url == url);
        match index {
            Some(i) => {
                self.mikan.remove(i);
                Ok(())
            }
            None => Err(eyre!("mikan with url {} not found", url)),
        }
    }

    pub fn add_collection(&mut self, collection: Collection) -> Result<()> {
        let maybe_pos = self
            .collections
            .iter()
            .position(|i| i.torrent_url == collection.torrent_url);
        if let Some(pos) = maybe_pos {
            self.collections[pos] = collection;
            Ok(())
        } else {
            self.collections.push(collection);
            Ok(())
        }
    }

    pub fn rm_collection(&mut self, url: &str) -> Result<()> {
        let index = self.collections.iter().position(|m| m.torrent_url == url);
        match index {
            Some(i) => {
                self.collections.remove(i);
                Ok(())
            }
            None => Err(eyre!("collection with torrent url {} not found", url)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::prelude::*;

    #[test]
    fn prase_config_str() {
        let a: Config = toml::from_str(
            r#"
        check_interval = 10
        res_api = "dmhy"

        [proxy]
        scheme = "http://127.0.0.1:10080"
        username = "admin"
        password = "123000"

        [downloader]
        type = "transmission"
        url = "https://192.168.1.1:8080/transmission/rpc"
        user = "admin"
        password = "123123"

        [link]
        enable = false
        path = "/downloads/link"
        dry_run = true
        notify = { type = "Ntfy", channel = "c" }

        [[mikan]]
        url = "u1"
        name = "n1"
        skip = [{url="1", title="1"}]
        title_contain = ["4"]
        external_subtitle = true
        ep_revise = -1

        [[mikan.extra]]
        title="2"
        url="3"

        [[mikan]]
        url = "u2"
        name = "n2"

        [[rules]]
        name = "约"
        keywords = ["约"]
        res_api = "dmhy"
        sub_group_id = 604
        sub_group_name = "c.c"
        publish_after = "2022-10-01T11:11:00"
        #res_type_id = 2
        #res_type_name = "动画"

        [[collections]]
        torrent_url = "u"
        name = "n"
        title = "1"
        season_folders = [{ folder = '', season = 1 }]
        special_mappings = [
            { file_name = "a.mkv", name = "bala", match_and_replace = true },
        ]
        external_subtitle = true
        "#,
        )
        .unwrap();
        assert_eq!(
            a,
            Config {
                rules: vec![Rule {
                    name: String::from("约"),
                    keywords: vec![String::from("约")],
                    res_api: ResApi::Dmhy,
                    sub_group_id: Some(604),
                    sub_group_name: Some(String::from("c.c")),
                    publish_after: Some(
                        NaiveDate::from_ymd_opt(2022, 10, 1)
                            .unwrap()
                            .and_hms_opt(11, 11, 0)
                            .unwrap()
                    ),
                    res_type_id: None,
                    res_type_name: None
                }],
                mikan: vec![
                    Mikan {
                        url: "u1".to_string(),
                        name: "n1".to_string(),
                        extra: vec![MikanItem {
                            title: "2".to_string(),
                            url: "3".to_string()
                        }],
                        skip: vec![MikanItem {
                            title: "1".to_string(),
                            url: "1".to_string()
                        }],
                        title_contain: vec![String::from("4")],
                        external_subtitle: true,
                        ep_revise: -1,
                    },
                    Mikan {
                        url: "u2".to_string(),
                        name: "n2".to_string(),
                        extra: vec![],
                        skip: vec![],
                        title_contain: vec![],
                        external_subtitle: false,
                        ep_revise: 0,
                    }
                ],
                downloader: Downloader::Transmission(TransmissionConfig {
                    url: String::from("https://192.168.1.1:8080/transmission/rpc"),
                    user: String::from("admin"),
                    password: String::from("123123"),
                }),
                res_api: ResApi::Dmhy,
                proxy: Some(Proxy {
                    scheme: String::from("http://127.0.0.1:10080"),
                    username: Some(String::from("admin")),
                    password: Some(String::from("123000"))
                }),
                check_interval: 10,
                link: Some(Link {
                    enable: false,
                    path: "/downloads/link".to_string(),
                    dry_run: true,
                    notify: Some(LinkNotify::Ntfy {
                        channel: "c".to_string()
                    })
                }),
                collections: vec![Collection {
                    torrent_url: "u".to_string(),
                    name: "n".to_string(),
                    title: "1".to_string(),
                    season_folders: vec![SeasonFolder {
                        season: 1,
                        folder: "".to_string()
                    }],
                    special_mappings: vec![SpecialMapping {
                        file_name: "a.mkv".to_string(),
                        name: "bala".to_string(),
                        match_and_replace: true,
                        matcher: Matcher::Off
                    }],
                    external_subtitle: true
                }]
            }
        );
    }

    #[test]
    fn prase_minimal_config_str() {
        let a: Config = toml::from_str(
            r#"
        check_interval = 10
        res_api = "dmhy"

        [downloader]
        type = "transmission"
        url = "https://192.168.1.1:8080/transmission/rpc"
        user = "admin"
        password = "123123"
        "#,
        )
        .unwrap();
        assert_eq!(
            a,
            Config {
                rules: vec![],
                mikan: vec![],
                downloader: Downloader::Transmission(TransmissionConfig {
                    url: String::from("https://192.168.1.1:8080/transmission/rpc"),
                    user: String::from("admin"),
                    password: String::from("123123"),
                }),
                res_api: ResApi::Dmhy,
                proxy: None,
                check_interval: 10,
                link: None,
                collections: vec![]
            }
        );
    }

    #[test]
    fn test_add_mikan() {
        let mut config = Config {
            rules: vec![],
            mikan: vec![],
            downloader: Downloader::Transmission(TransmissionConfig {
                url: String::from("https://192.168.1.1:8080/transmission/rpc"),
                user: String::from("admin"),
                password: String::from("123123"),
            }),
            res_api: ResApi::Dmhy,
            proxy: None,
            check_interval: 10,
            link: None,
            collections: vec![],
        };
        config
            .add_mikan(Mikan {
                url: "u1".to_string(),
                name: "n1".to_string(),
                extra: vec![MikanItem {
                    title: "2".to_string(),
                    url: "3".to_string(),
                }],
                skip: vec![MikanItem {
                    title: "1".to_string(),
                    url: "1".to_string(),
                }],
                title_contain: vec![String::from("4")],
                external_subtitle: true,
                ep_revise: -2,
            })
            .unwrap();

        let expected_config: Config = toml::from_str(
            r#"
        check_interval = 10
        res_api = "dmhy"

        [downloader]
        type = "transmission"
        url = "https://192.168.1.1:8080/transmission/rpc"
        user = "admin"
        password = "123123"

        [[mikan]]
        url = "u1"
        name = "n1"
        title_contain = ["4"]
        external_subtitle = true
        extra = [{title="2", url="3"}]
        skip = [{title="1", url="1"}]
        ep_revise = -2
        "#,
        )
        .unwrap();
        assert_eq!(config, expected_config);
    }
}
