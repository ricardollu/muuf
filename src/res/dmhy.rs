use async_trait::async_trait;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use color_eyre::eyre::{Error, Result};
use scraper::{Html, Selector};

use super::{get_info_hash_from_magnet, ApiServer, Res};

pub struct Dmhy {
    pub base_uri: String,
}

#[async_trait]
impl ApiServer for Dmhy {
    async fn sub_groups(&self) -> Result<Vec<(i32, String)>, Error> {
        let uri = format!(
            "{}/topics/advanced-search?team_id=0&sort_id=0&orderby=",
            self.base_uri
        );
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

    async fn res_types(&self) -> Result<Vec<(i32, String)>, Error> {
        let uri = format!(
            "{}/topics/advanced-search?team_id=0&sort_id=0&orderby=",
            self.base_uri
        );
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

    async fn res_list(
        &self,
        keywords: &[String],
        sub_group_id: Option<i32>,
        res_type_id: Option<i32>,
        publish_after: Option<DateTime<Local>>,
    ) -> Result<(Vec<Res>, bool)> {
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
            .any(|e| e.inner_html() == "下一頁");
        let mut res_list = document
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

                let seeding = td5
                    .first_child()
                    .unwrap()
                    .first_child()
                    .unwrap()
                    .value()
                    .as_text()
                    .unwrap()
                    .to_string();
                let leeching = td6
                    .first_child()
                    .unwrap()
                    .first_child()
                    .unwrap()
                    .value()
                    .as_text()
                    .unwrap()
                    .to_string();
                let finished = td7
                    .first_child()
                    .unwrap()
                    .value()
                    .as_text()
                    .unwrap()
                    .to_string();
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
                    api: crate::config::ResApi::Dmhy,
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
                    publish_date: str_to_time(
                        td0.select(&Selector::parse("span").unwrap())
                            .next()
                            .unwrap()
                            .text()
                            .collect::<String>()
                            .trim(),
                    ),
                    page_url: format!(
                        "{}{}",
                        self.base_uri,
                        td2_a_last.value().attr("href").unwrap()
                    ),
                    magnet,
                    info_hash,
                    seeding,
                    leeching,
                    finished,
                }
            })
            .filter(|res| {
                if let Some(i) = res_type_id {
                    if res.type_id != i {
                        return false;
                    }
                }
                if let Some(d) = publish_after {
                    if res.publish_date < d {
                        return false;
                    }
                }
                true
            })
            .collect::<Vec<Res>>();
        res_list.reverse();

        Ok((res_list, has_more))
    }
}

fn str_to_time(str: &str) -> DateTime<Local> {
    Local
        .from_local_datetime(&NaiveDateTime::parse_from_str(str, "%Y/%m/%d %R").unwrap())
        .unwrap()
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};

    use super::*;

    #[test]
    fn test_str_to_time() {
        let str = "2021/08/26 17:17";
        let time = str_to_time(str);
        assert_eq!(
            time,
            Local.with_ymd_and_hms(2021, 8, 26, 17, 17, 0).unwrap()
        );
        assert_ne!(
            time,
            Local.with_ymd_and_hms(2021, 8, 26, 17, 17, 1).unwrap()
        );
    }
}
