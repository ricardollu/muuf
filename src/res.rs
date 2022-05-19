use anyhow::Result;
use async_trait::async_trait;
use data_encoding::{BASE32, HEXLOWER};
use regex::Regex;
use scraper::{Html, Selector};

pub fn get_res_api(res_config: &crate::config::ResApi) -> impl ApiServer {
    match res_config {
        crate::config::ResApi::Dmhy => Dmhy {
            base_uri: String::from("https://share.dmhy.org"),
        },
    }
}

#[async_trait]
pub trait ApiServer {
    async fn sub_groups(&self) -> Result<Vec<(i32, String)>>;
    async fn res_types(&self) -> Result<Vec<(i32, String)>>;
    async fn res_list(&self, keywords: &[String], subgroup_id: Option<i32>, res_type_id: Option<i32>) -> Result<(Vec<Res>, bool)>;
}

struct Dmhy {
    base_uri: String,
}

#[async_trait]
impl ApiServer for Dmhy {
    async fn sub_groups(&self) -> Result<Vec<(i32, String)>, anyhow::Error> {
        let uri = format!("{}/topics/advanced-search?team_id=0&sort_id=0&orderby=", self.base_uri);
        let text = crate::CLIENT.get(uri).send().await?.text().await?;

        let document = Html::parse_document(&text);
        let selector = Selector::parse("select#AdvSearchTeam option").unwrap();
        let mut groups = Vec::new();
        for e in document.select(&selector) {
            let group_id = match e.value().attr("value") {
                Some(id) => id.parse::<i32>().unwrap(),
                None => -1,
            };
            let group_name = e.inner_html();
            if group_id > 0 {
                groups.push((group_id, group_name));
            }
        }

        Ok(groups)
    }

    async fn res_types(&self) -> Result<Vec<(i32, String)>, anyhow::Error> {
        let uri = format!("{}/topics/advanced-search?team_id=0&sort_id=0&orderby=", self.base_uri);
        let text = crate::CLIENT.get(uri).send().await?.text().await?;

        let document = Html::parse_document(&text);
        let selector = Selector::parse("select#AdvSearchSort option").unwrap();
        let mut types = Vec::new();
        for e in document.select(&selector) {
            let type_id = match e.value().attr("value") {
                Some(id) => id.parse::<i32>().unwrap(),
                None => -1,
            };
            let type_name = e.inner_html();
            if type_id > 0 {
                types.push((type_id, type_name));
            }
        }

        Ok(types)
    }

    async fn res_list(&self, keywords: &[String], sub_group_id: Option<i32>, res_type_id: Option<i32>) -> Result<(Vec<Res>, bool)> {
        let uri = format!(
            "{}/topics/list/page/1?keyword={}&sort_id={}&team_id={}&order=date-desc",
            self.base_uri,
            keywords.join("+"),
            res_type_id.unwrap_or(0),
            sub_group_id.unwrap_or(0)
        );
        let unknown_subgroup_id = -1;
        let unknown_subgroup_name = "未知字幕组";

        let text = crate::CLIENT.get(uri).send().await?.text().await?;
        let document = Html::parse_document(&text);
        let has_more = document
            .select(&Selector::parse("div.nav_title > a").unwrap())
            .into_iter()
            .any(|e| e.inner_html() == "下一頁");
        let res_list = document
            .select(&Selector::parse("table#topic_list tbody tr").unwrap())
            .map(|tr| {
                let td_selector = Selector::parse("td").unwrap();
                let mut td_iter = tr.select(&td_selector);
                let td0 = td_iter.next().unwrap();
                let td1 = td_iter.next().unwrap();
                let td2 = td_iter.next().unwrap();
                let td3 = td_iter.next().unwrap();
                let td4 = td_iter.next().unwrap();
                let td5 = td_iter.next().unwrap();
                let td6 = td_iter.next().unwrap();
                let td7 = td_iter.next().unwrap();
                let a_selector = Selector::parse("a").unwrap();
                let td1_a0 = td1.select(&a_selector).next().unwrap();
                let td2_a_count = td2.select(&a_selector).count();
                let td2_a0 = td2.select(&a_selector).next().unwrap();
                let td2_a_last = td2.select(&a_selector).last().unwrap();
                let td3_a0 = td3.select(&a_selector).next().unwrap();

                let seeding = td5.first_child().unwrap().first_child().unwrap().value().as_text().unwrap().to_string();
                let leeching = td6.first_child().unwrap().first_child().unwrap().value().as_text().unwrap().to_string();
                let finished = td7.first_child().unwrap().value().as_text().unwrap().to_string();
                let magnet = td3_a0.value().attr("href").unwrap().to_string();
                let info_hash = get_info_hash_from_magnet(&magnet).unwrap();
                // prefer tr.bangumi.moe
                let mut magnet_url = magnet_url::Magnet::new(&magnet).unwrap();
                magnet_url.tr.sort_by(|a, _| {
                    if a.contains("tr.bangumi.moe%3A6969") {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    }
                });
                magnet_url.xt = Some(info_hash.to_string());
                let magnet = magnet_url.to_string();
                Res {
                    title: td2_a_last.text().collect::<String>().trim().to_string(),
                    type_id: td1_a0
                        .value()
                        .attr("href")
                        .unwrap()
                        .replace("/topics/list/sort_id/", "")
                        .parse::<i32>()
                        .unwrap(),
                    type_name: td1_a0.text().collect::<String>().trim().to_string(),
                    sub_group_id: if td2_a_count != 2 {
                        unknown_subgroup_id
                    } else {
                        td2_a0
                            .value()
                            .attr("href")
                            .unwrap()
                            .replace("/topics/list/team_id/", "")
                            .parse::<i32>()
                            .unwrap()
                    },
                    sub_group_name: if td2_a_count != 2 {
                        unknown_subgroup_name.to_string()
                    } else {
                        td2_a0.text().collect::<String>().trim().to_string()
                    },
                    file_size: td4.text().collect::<String>().trim().to_string(),
                    publish_date: td0
                        .select(&Selector::parse("span").unwrap())
                        .next()
                        .unwrap()
                        .text()
                        .collect::<String>()
                        .trim()
                        .to_string(),
                    page_url: format!("{}{}", self.base_uri, td2_a_last.value().attr("href").unwrap()),
                    magnet,
                    info_hash,
                    seeding,
                    leeching,
                    finished,
                }
            })
            .filter(|res| if let Some(i) = res_type_id { res.type_id == i } else { true })
            .collect::<Vec<Res>>();

        Ok((res_list, has_more))
    }
}

pub struct Res {
    pub title: String,
    pub type_id: i32,
    pub type_name: String,
    pub sub_group_id: i32,
    pub sub_group_name: String,
    pub file_size: String,
    pub page_url: String,
    pub magnet: String,
    pub info_hash: String,
    pub publish_date: String,
    pub seeding: String,
    pub leeching: String,
    pub finished: String,
}

fn get_info_hash_from_magnet(magnet: &str) -> Result<String> {
    let regex = Regex::new(r"xt=urn:(sha1|btih|ed2k|aich|kzhash|md5|tree:tiger):([A-Za-z0-9]+)").unwrap();
    match regex.captures(magnet) {
        Some(captures) => {
            let hash_type = captures.get(1).unwrap().as_str();
            if hash_type != "btih" {
                return Err(anyhow::anyhow!("can't handle magnet other than btih"));
            }
            let hash = captures.get(2).unwrap().as_str();
            let hash_len = hash.len();
            if hash_len == 32 {
                Ok(HEXLOWER.encode(BASE32.decode(hash.as_bytes()).unwrap().as_ref()))
            } else if hash_len == 40 {
                Ok(hash.to_lowercase())
            } else {
                Err(anyhow::anyhow!("invalid hash len {}", hash_len))
            }
        }
        None => Err(anyhow::anyhow!("invalid magnet scheme")),
    }
}
