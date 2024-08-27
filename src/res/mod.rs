use async_trait::async_trait;
use chrono::{DateTime, Local};
use color_eyre::eyre::{eyre, Result};
use data_encoding::{BASE32, HEXLOWER};
use regex::Regex;

mod dmhy;

pub fn get_res_api(res_config: &crate::config::ResApi) -> impl ApiServer {
    match res_config {
        crate::config::ResApi::Dmhy => dmhy::Dmhy {
            base_uri: String::from("https://share.dmhy.org"),
        },
    }
}

#[async_trait]
pub trait ApiServer {
    async fn sub_groups(&self) -> Result<Vec<(i32, String)>>;
    async fn res_types(&self) -> Result<Vec<(i32, String)>>;
    async fn res_list(
        &self,
        keywords: &[String],
        subgroup_id: Option<i32>,
        res_type_id: Option<i32>,
        publish_after: Option<DateTime<Local>>,
    ) -> Result<(Vec<Res>, bool)>;
}

pub struct Res {
    pub title: String,
    pub api: crate::config::ResApi,
    pub type_id: i32,
    pub type_name: String,
    pub sub_group_id: i32,
    pub sub_group_name: String,
    pub file_size: String,
    pub page_url: String,
    pub magnet: String,
    pub info_hash: String,
    pub publish_date: DateTime<Local>,
    pub seeding: String,
    pub leeching: String,
    pub finished: String,
}

fn get_info_hash_from_magnet(magnet: &str) -> Result<String> {
    let regex =
        Regex::new(r"xt=urn:(sha1|btih|ed2k|aich|kzhash|md5|tree:tiger):([A-Za-z0-9]+)").unwrap();
    match regex.captures(magnet) {
        Some(captures) => {
            let hash_type = captures.get(1).unwrap().as_str();
            if hash_type != "btih" {
                return Err(eyre!("can't handle magnet other than btih"));
            }
            let hash = captures.get(2).unwrap().as_str();
            let hash_len = hash.len();
            if hash_len == 32 {
                Ok(HEXLOWER.encode(BASE32.decode(hash.as_bytes()).unwrap().as_ref()))
            } else if hash_len == 40 {
                Ok(hash.to_lowercase())
            } else {
                Err(eyre!("invalid hash len {}", hash_len))
            }
        }
        None => Err(eyre!("invalid magnet scheme")),
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
    use chrono::prelude::*;

    #[test]
    fn test_time_compare() {
        let time1 = Local
            .from_local_datetime(
                &NaiveDateTime::parse_from_str("2022/10/01 10:01", "%Y/%m/%d %R").unwrap(),
            )
            .unwrap();
        let time2 = Local
            .from_local_datetime(
                &NaiveDateTime::parse_from_str("2022/10/01 10:02", "%Y/%m/%d %R").unwrap(),
            )
            .unwrap();
        assert!(time1 < time2)
    }
}
