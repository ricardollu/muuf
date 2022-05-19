use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, PartialEq, Deserialize)]
pub struct Config {
    pub rules: Vec<Rule>,
    pub downloader: Downloader,
    pub res_api: ResApi,
    pub proxy: Option<Proxy>,
    pub interval: u64,
}

#[derive(Debug, PartialEq, Deserialize, Hash, Eq)]
#[serde(rename_all(deserialize = "lowercase"))]
pub enum ResApi {
    Dmhy,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Proxy {
    pub scheme: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all(deserialize = "lowercase"))]
pub enum Downloader {
    Transmission(TransmissionConfig),
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct TransmissionConfig {
    pub url: String,
    pub user: String,
    pub password: String,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Rule {
    pub name: String,
    pub keywords: Vec<String>,
    pub res_api: ResApi,
    pub sub_group_id: Option<i32>,
    pub sub_group_name: Option<String>,
    pub res_type_id: Option<i32>,
    pub res_type_name: Option<String>,
}

pub fn load() -> Result<Config> {
    toml::from_slice(&read_config()?).map_err(|e| anyhow::anyhow!(e))
}

/// TODO where to read the config file?
fn read_config() -> Result<Vec<u8>> {
    std::fs::read("config.toml").map_err(|e| anyhow::anyhow!(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let a: Config = toml::from_str(
            r#"
        interval = 10
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

        [[rules]]
        name = "约会大作战"
        keywords = ["约会"]
        res_api = "dmhy"
        sub_group_id = 604
        sub_group_name = "c.c动漫"
        #res_type_id = 2
        #res_type_name = "动画"
        "#,
        )
        .unwrap();
        assert_eq!(
            a,
            Config {
                rules: vec![Rule {
                    name: String::from("约会大作战"),
                    keywords: vec![String::from("约会")],
                    sub_group_id: Some(604),
                    sub_group_name: Some(String::from("c.c动漫")),
                    res_api: ResApi::Dmhy,
                    res_type_id: None,
                    res_type_name: None
                }],
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
                interval: 10,
            }
        );
    }
}
