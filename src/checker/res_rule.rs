use color_eyre::eyre::Result;

use crate::{
    config::Rule,
    dl::{Client, Torrent},
    res::{self, ApiServer},
};

pub async fn check_res_rule(
    rule: &Rule,
    dl_client: &mut dyn Client,
    dl_server_torrents: &[Torrent],
    added_torrent_hashs: &mut Vec<String>,
) -> Result<()> {
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

    Ok(())
}
