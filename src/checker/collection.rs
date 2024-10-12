use std::{ffi::OsStr, fs, path::Path};

use base64::{engine::general_purpose, Engine};
use color_eyre::eyre::{bail, eyre, Result};

use crate::{
    config::{Collection, Link, Matcher, SeasonFolder, SpecialMapping},
    dl::{Client, Torrent},
    get_url_bytes,
    parser::{self},
    VIDEO_EXTS,
};

pub async fn check_collection(
    collection: &Collection,
    dl_client: &mut dyn Client,
    dl_server_torrents: &[Torrent],
    added_torrent_hashs: &mut Vec<String>,
    maybe_link: &Option<Link>,
) -> Result<()> {
    let Collection {
        name,
        torrent_url,
        title,
        season_folders,
        special_mappings,
        external_subtitle,
    } = collection;
    let bytes = get_url_bytes(torrent_url).await?;
    let torrent = lava_torrent::torrent::v1::Torrent::read_from_bytes(&bytes)?;
    // If the torrent contains only 1 file then files is None.
    if torrent.files.is_none() {
        bail!("不是多文件种子: {}", title);
    }

    let some_server_torrent = dl_server_torrents
        .iter()
        .find(|t| t.hash == torrent.info_hash());
    if let Some(server_torrent) = some_server_torrent {
        if let Some(link_config) = maybe_link {
            if link_config.enable && server_torrent.percent_done >= 1.0 {
                for file in torrent
                    .files
                    .as_ref()
                    .ok_or_else(|| eyre!("torrent has only one file: {title}"))?
                {
                    let file_name_from_torrent = file
                        .path
                        .file_name()
                        .and_then(OsStr::to_str)
                        .ok_or_else(|| eyre!("get file_name & to_str failed: {:?}", file.path))?;
                    let file_suffix = file
                        .path
                        .extension()
                        .and_then(OsStr::to_str)
                        .ok_or_else(|| eyre!("get ext & to_str failed: {:?}", file.path))?;
                    if VIDEO_EXTS.iter().all(|ext| ext != &file_suffix) {
                        continue;
                    }
                    let file_stem = file
                        .path
                        .file_stem()
                        .and_then(OsStr::to_str)
                        .ok_or_else(|| eyre!("get file_stem & to_str failed: {:?}", file.path))?;

                    let season;
                    let link_file_name;

                    if let Some(SpecialMapping { name, matcher, .. }) =
                        special_mappings.iter().find(|sm| match &sm.matcher {
                            Matcher::Off => sm.file_name == file_name_from_torrent,
                            Matcher::On(regex) => regex.is_match(file_name_from_torrent),
                        })
                    {
                        season = &0_u8;
                        match matcher {
                            Matcher::Off => link_file_name = name.to_string(),
                            Matcher::On(regex) => {
                                if let Some(captures) = regex.captures(file_name_from_torrent) {
                                    let mut name = name.to_string();
                                    for (i, cap) in captures.iter().enumerate() {
                                        if i == 0 {
                                            continue;
                                        }
                                        if let Some(cap) = cap {
                                            name = name.replace(&format!("{{{i}}}"), cap.as_str())
                                        }
                                    }
                                    link_file_name = name;
                                } else {
                                    link_file_name = name.to_string();
                                }
                            }
                        }
                    } else {
                        let parent =
                            file.path.parent().and_then(Path::to_str).ok_or_else(|| {
                                eyre!("get parent & to_str failed: {:?}", file.path)
                            })?;
                        let maybe_season = if let Some(SeasonFolder { season, .. }) =
                            season_folders.iter().find(|sf| sf.folder == parent)
                        {
                            Some(season)
                        } else {
                            None
                        };
                        if maybe_season.is_none() {
                            continue;
                        }
                        season = maybe_season.unwrap(); // is_none checked
                        let maybe_real_ep = parser::process(file_name_from_torrent);
                        if maybe_real_ep.is_err() {
                            println!(
                                "{file_name_from_torrent} 解析失败: {}",
                                maybe_real_ep.unwrap_err() // is_err checked
                            );
                            continue;
                        }
                        let real_ep = maybe_real_ep.unwrap(); // is_err checked
                        link_file_name = real_ep.link_file_name_with_season(name, season);
                    }

                    let path = parser::link_path(name, season);
                    let full_path = format!("{}/{path}", &link_config.path);
                    let full_file_name = format!("{}.{file_suffix}", link_file_name);
                    let link = format!("{full_path}/{full_file_name}");
                    if !Path::new(&link).exists() {
                        let original = format!(
                            "{}/{}/{}",
                            &server_torrent.download_dir,
                            &torrent.name,
                            file.path.to_str().ok_or_else(|| {
                                eyre!("get path & to_str failed: {:?}", file.path)
                            })?
                        );
                        if link_config.dry_run {
                            println!("准备链接{link} <- {original}",);
                        } else {
                            fs::create_dir_all(&full_path)?;
                            match fs::hard_link(&original, &link) {
                                Ok(_) => {
                                    println!(
                                        "创建链接{link} <- {}/{file_name_from_torrent}",
                                        &torrent.name
                                    );
                                }
                                Err(e) => println!("硬链接失败: {} 当{link} <- {original}", e),
                            }
                        }
                    }

                    // 外挂字幕
                    if *external_subtitle {
                        parser::link_external_subtitle(
                            &torrent,
                            file_stem,
                            &full_path,
                            &link_file_name,
                            link_config,
                            server_torrent,
                        )?
                    }
                }
            }
        }

        return Ok(());
    }
    if added_torrent_hashs.contains(&torrent.info_hash()) {
        println!("{} 刚刚已经被加入下载了", title);
        return Ok(());
    }
    dl_client
        .torrent_add_by_meta(general_purpose::STANDARD.encode(bytes), name)
        .await?;
    added_torrent_hashs.push(torrent.info_hash());
    println!("加入下载列表: {}", title);

    Ok(())
}
