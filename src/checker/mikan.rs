use std::{
    fs,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose, Engine};
use color_eyre::eyre::Result;

use crate::{
    config::{Link, Mikan},
    dl::{Client, Torrent},
    get_url_bytes,
    parser::{self, Episode},
    rss::parse_mikan,
    VIDEO_EXTS,
};

pub async fn check_mikan(
    m: &Mikan,
    dl_client: &mut dyn Client,
    dl_server_torrents: &[Torrent],
    added_torrent_hashs: &mut Vec<String>,
    maybe_link: &Option<Link>,
) -> Result<()> {
    let mut ts = parse_mikan(&m.url).await?;
    for e in &m.extra {
        ts.push((e.title.clone(), e.url.clone(), get_url_bytes(&e.url).await));
    }
    let ts = ts
        .into_iter()
        .filter(|(_, _, maybe_bytes)| maybe_bytes.is_ok())
        .map(|(a, b, c)| (a, b, c.unwrap()));

    for (title, url, bytes) in ts {
        if m.skip.iter().any(|s| {
            if s.title.trim() == "" {
                url.trim() == s.url.trim()
            } else if s.url.trim() == "" {
                title.trim() == s.title.trim()
            } else {
                title.trim() == s.title.trim() && url.trim() == s.url.trim()
            }
        }) {
            continue;
        }
        if !m.title_contain.iter().all(|s| title.contains(s)) {
            continue;
        }

        if title.contains("合集") {
            // println!("跳过合集: {} ", title);
            continue;
        }

        let torrent = lava_torrent::torrent::v1::Torrent::read_from_bytes(&bytes)?;
        let pathbuf_torrent_name = PathBuf::from(&torrent.name);
        // If the torrent contains only 1 file then files is None.
        let (file_name_from_torrent, file_stem, storage_path) = if torrent.files.is_some() {
            let mut some_file_name_from_torrent = None;
            for file in torrent.files.as_ref().unwrap() {
                let file_suffix = file.path.extension().unwrap().to_str().unwrap();
                if VIDEO_EXTS.iter().any(|ext| ext == &file_suffix) {
                    some_file_name_from_torrent = Some((
                        file.path.file_name().unwrap().to_str().unwrap(),
                        file.path.file_stem().unwrap().to_str().unwrap(),
                        format!("{}/", &torrent.name),
                    ));
                    break;
                }
            }
            if let Some(n) = some_file_name_from_torrent {
                n
            } else {
                println!("跳过没有视频文件的多文件种子: {}", title);
                continue;
            }
        } else {
            (
                torrent.name.as_str(),
                pathbuf_torrent_name.file_stem().unwrap().to_str().unwrap(),
                "".to_string(),
            )
        };

        let some_server_torrent = dl_server_torrents
            .iter()
            .find(|t| t.hash == torrent.info_hash());
        if let Some(server_torrent) = some_server_torrent {
            if let Some(link_config) = maybe_link {
                if link_config.enable && (server_torrent.percent_done >= 1.0 || link_config.dry_run)
                {
                    // If the torrent contains only 1 file then name is the file name. Otherwise it’s the suggested root directory’s name.
                    // let file_name_from_torrent = &torrent.name;
                    let file_suffix = file_name_from_torrent.split('.').last().unwrap();
                    let ep = process(&title, m)?;
                    let name = ep.name(Some(&m.name))?;
                    let path = ep.link_path(&name);
                    let link_file_name = ep.link_file_name(&name);

                    let full_path = format!("{}/{path}", &link_config.path);
                    let full_file_name = format!("{link_file_name}.{file_suffix}");
                    let link = format!("{full_path}/{full_file_name}");
                    if !Path::new(&link).exists() {
                        if link_config.dry_run {
                            println!("准备链接{link} <- {storage_path}{file_name_from_torrent}");
                        } else {
                            fs::create_dir_all(&full_path)?;
                            let original = format!(
                                "{}/{storage_path}{file_name_from_torrent}",
                                &server_torrent.download_dir,
                            );
                            match fs::hard_link(&original, &link) {
                                Ok(_) => {
                                    println!(
                                        "创建链接{link} <- {storage_path}{file_name_from_torrent}"
                                    );
                                    // send notify when link success
                                    if let Some(notify) = &link_config.notify {
                                        notify.link_success(&link_file_name).await?;
                                    }
                                }
                                Err(e) => println!("硬链接失败: {} 当{link} <- {original}", e),
                            }
                        }
                    }

                    // 外挂字幕
                    if m.external_subtitle {
                        parser::link_external_subtitle(
                            &torrent,
                            file_stem,
                            &full_path,
                            &link_file_name,
                            link_config,
                            server_torrent,
                        )?;
                    }
                }
            }

            continue;
        }
        if added_torrent_hashs.contains(&torrent.info_hash()) {
            println!("{} 刚刚已经被加入下载了", title);
            continue;
        }
        dl_client
            .torrent_add_by_meta(general_purpose::STANDARD.encode(bytes), &m.name)
            .await?;
        added_torrent_hashs.push(torrent.info_hash());
        println!("加入下载列表: {}", title)
    }

    Ok(())
}

fn process(title: &str, m: &Mikan) -> Result<Episode> {
    let mut ep = parser::process(title)?;
    ep.revise_ep(&m.ep_revise);
    Ok(ep)
}

#[cfg(test)]
mod tests {
    use parser::Ep;

    use super::*;

    #[test]
    fn test_process() {
        let mikan = toml::from_str::<Mikan>(
            r#"
            name = ""
            url = ""
            ep_revise = -48
            "#,
        )
        .unwrap();
        let ep = process("[Up to 21°C] 关于我转生变成史莱姆这档事 第三季 / Tensei shitara Slime Datta Ken 3rd Season - 49 (Baha 1920x1080 AVC AAC MP4)", &mikan);
        assert!(matches!(ep, Ok(Episode::Ep(Ep {episode, ..})) if episode == 1 ))
    }
}
