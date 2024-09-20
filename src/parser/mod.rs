use std::iter::Iterator;
use std::sync::LazyLock;
use std::{collections::HashMap, path};

use color_eyre::eyre::{bail, eyre, Result};
use lava_torrent::torrent::v1::Torrent;
use regex::Regex;

use crate::VIDEO_EXTS;
use crate::{
    config,
    dl::{self},
};

/*
   本文件代码初版翻译自
   https://github.com/EstrellaXD/Auto_Bangumi/blob/main/backend/src/module/parser/analyser/raw_parser.py

   作者: EstrellaXD
   协议: MIT
*/

static RESOLUTION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"1080|720|2160|4K").unwrap());
static SOURCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"B-Global|[Bb]aha|[Bb]ilibili|AT-X|Web").unwrap());
static SUB_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[简繁日字幕]|CH|BIG5|GB").unwrap());
/*
   这个正则表达式匹配所有不是字母、数字、下划线、空格、汉字、日文假名、片假名和连字符的字符。
   其中，\w 匹配字母、数字和下划线；\s 匹配空格；\u4e00-\u9fff 匹配汉字；\u3040-\u309f 匹配日文假名；\u30a0-\u30ff 匹配片假名；- 匹配连字符。
   [^\w\s\u4e00-\u9fff\u3040-\u309f\u30a0-\u30ff-] 表示匹配除了这些字符以外的所有字符
*/
static PREFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^\w\s\u4e00-\u9fff\u3040-\u309f\u30a0-\u30ff-]").unwrap());

static CHINESE_NUMBER_MAP: LazyLock<HashMap<&'static str, u8>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert("一", 1);
    map.insert("二", 2);
    map.insert("三", 3);
    map.insert("四", 4);
    map.insert("五", 5);
    map.insert("六", 6);
    map.insert("七", 7);
    map.insert("八", 8);
    map.insert("九", 9);
    map.insert("十", 10);
    map
});

#[derive(Debug, PartialEq)]
enum Lang {
    En,
    Zh,
    Jp,
    Other,
}

fn find_lang(str: &str) -> Lang {
    if Regex::new(r"[\u0800-\u4e00]{2,}").unwrap().is_match(str) {
        Lang::Jp
    } else if Regex::new(r"[\u4e00-\u9fa5]{2,}").unwrap().is_match(str) {
        Lang::Zh
    } else if Regex::new(r"[a-zA-Z]{2,}").unwrap().is_match(str) {
        Lang::En
    } else {
        Lang::Other
    }
}

// (name_en, name_zh, name_jp)
fn name_process(name: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut name_en = None;
    let mut name_zh = None;
    let mut name_jp = None;
    // 去除仅限港澳台字样 僅限港澳台地區
    let name_v1 = Regex::new(r"[(（][仅僅]限港澳台地[区區][）)]")
        .unwrap()
        .replace_all(name.trim(), "");
    // 用 / 或 两个空格 或 -跟两个空格 分割字符串
    let split = Regex::new(r"/|\s{2}|-\s{2}")
        .unwrap()
        .split(&name_v1)
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<&str>>();
    // 如果只有一个分割结果，那么就使用 _ 或 空格-空格 分割
    let split = if split.len() == 1 {
        if Regex::new("_{1}").unwrap().is_match(&name_v1) {
            Regex::new(r"_")
                .unwrap()
                .split(&name_v1)
                .collect::<Vec<&str>>()
        } else if Regex::new(" - {1}").unwrap().is_match(&name_v1) {
            Regex::new(r" - ")
                .unwrap()
                .split(&name_v1)
                .collect::<Vec<&str>>()
        } else {
            split
        }
    } else {
        split
    };
    // 如果分割结果只有一个，那么就用空格分割
    let split = if split.len() == 1 {
        split
            .first()
            .unwrap()
            .split(' ')
            .fold(Vec::<String>::new(), |mut acc, item| {
                // 如果最后一个元素和当前元素的语言相同，那么就合并
                match acc.last_mut() {
                    Some(last) => {
                        if find_lang(last) == find_lang(item) {
                            *last = format!("{} {}", last, item);
                        } else {
                            acc.push(item.to_string());
                        }
                    }
                    None => acc.push(item.to_string()),
                };
                acc
            })
    } else {
        split.iter().map(|s| s.to_string()).collect()
    };
    // 处理分割结果
    for item in split {
        match find_lang(&item) {
            Lang::En => {
                if name_en.is_none() {
                    name_en = Some(item.trim().to_owned())
                }
            }
            Lang::Zh => {
                if name_zh.is_none() {
                    name_zh = Some(item.trim().to_owned())
                }
            }
            Lang::Jp => {
                if name_jp.is_none() {
                    name_jp = Some(item.trim().to_owned())
                }
            }
            Lang::Other => {}
        }
    }

    (name_en, name_zh, name_jp)
}

fn find_tags_from_iter<'a, T>(iter: T) -> (Option<String>, Option<String>, Option<String>)
where
    T: IntoIterator<Item = &'a str>,
{
    let (mut sub, mut resolution, mut source) = (None, None, None);

    for e in iter {
        if sub.is_none() && SUB_RE.is_match(e) {
            sub = Some(e.to_string());
        } else if resolution.is_none() && RESOLUTION_RE.is_match(e) {
            resolution = Some(e.to_string());
        } else if source.is_none() && SOURCE_RE.is_match(e) {
            source = Some(e.to_string());
        }
    }
    // clean_sub
    let sub = sub.map(|sub| {
        Regex::new(r"_MP4|_MKV")
            .unwrap()
            .replace_all(&sub, "")
            .to_string()
    });

    (sub, resolution, source)
}

/**
 * 处理单集标题
 * 标题格式: [group] name [season] [ep] [sub] [dpi] [source]
 * 可以在 name 里包含 season 和 ep
 * 独立ep块优先级更高
 * 在其他块里寻找 语言sub, 清晰度res, 来源source
 */
pub fn process(title: &str) -> Result<Episode> {
    let original_title = title;
    //  1. 去除分割符号，分成多个小块
    let title = title.trim().replace('【', "[").replace('】', "]");
    let mut blocks = Regex::new(r"[\[\]]")
        .unwrap()
        .split(&title)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
    // 只有group和其他两块，把第二块重新分割
    if blocks.len() == 2 {
        let block_two = blocks.remove(1);
        let split_reg = Regex::new(r"[ -]").unwrap();
        blocks.extend(
            split_reg
                .split(&block_two)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
        );
    }
    // 去除x月新番
    let boring_block_reg1 = Regex::new(r"新番|月?番").unwrap();
    blocks.retain(|s| {
        let ss = PREFIX_RE.replace_all(s, "");
        !(boring_block_reg1.is_match(&ss) && ss.len() <= 11)
    });

    // println!("{:?}", blocks);

    // 3. 块1: group
    let group = blocks.first().unwrap().to_string();

    // [Season]
    let mut maybe_season: Option<u8> = None;
    let season_search_reg =
        Regex::new(r"S(\d{1,2})|Season (\d{1,2})|[第 ](.)(?:部分|[季期部])").unwrap();

    // ep
    let ep_reg =
        Regex::new(r"^(?:(\d+)|(\d+).?[vV](?<version>\d)|第?(\d+)[话話集]|(\d+).?END)$").unwrap();
    let mut maybe_ep: Option<u32> = None;
    let mut maybe_name_block_end_index: Option<usize> = None; // ep块的前一个块是name块 或者 ep包含在name块中

    for (index, block) in blocks[1..].iter_mut().enumerate() {
        // 没有season时尝试解析
        if maybe_season.is_none() {
            maybe_season = if let Some(Some(m)) = season_search_reg
                .captures(block)
                .and_then(|c| c.iter().skip(1).find(|s| s.is_some()))
            {
                match m.as_str().parse::<u8>() {
                    Ok(season_int) => Some(season_int),
                    Err(_) => CHINESE_NUMBER_MAP.get(m.as_str()).copied(),
                }
            } else {
                None
            };

            // 去除Season信息
            if maybe_season.is_some() {
                *block = season_search_reg.replace_all(block, "").to_string();
            }
        }

        // 尝试解析ep
        if maybe_ep.is_none() {
            maybe_ep = if let Some(Some(s)) = ep_reg
                .captures(block)
                .and_then(|c| c.iter().skip(1).find(|s| s.is_some()))
            {
                Some(s.as_str().parse::<u32>().unwrap())
            } else {
                None
            };

            if maybe_ep.is_some() {
                maybe_name_block_end_index = Some(index);
            }
        }
    }

    // 重新找，因为独立ep块的优先级高
    if maybe_ep.is_none() {
        let ep_from_name_reg =
            Regex::new(r" -? ?(?:(\d+)|第(\d+)[话話集]|[Ee][Pp]?(\d+))(?:.?[vV](?<version>\d))?")
                .unwrap();
        for (index, block) in blocks[1..].iter_mut().enumerate() {
            maybe_ep = if let Some(Some(s)) = ep_from_name_reg
                .captures(block)
                .and_then(|c| c.iter().skip(1).find(|s| s.is_some()))
            {
                Some(s.as_str().parse::<u32>().unwrap())
            } else {
                None
            };

            if maybe_ep.is_some() {
                // 去除ep信息, 只保留第一个匹配之前的字符
                let remain = block[ep_from_name_reg.find(block).unwrap().end()..].trim();
                if !remain.is_empty() {
                    // println!("remain:{}", &remain);
                    bail!("can't understand episode number for title: {}", title)
                }
                *block = block[..ep_from_name_reg.find(block).unwrap().start()].to_string();
                maybe_name_block_end_index = Some(index + 1);
                break;
            }
        }
    }

    if maybe_ep.is_none() {
        return Ok(Episode::Sp {
            name: original_title.to_string(),
        });
    }
    if maybe_name_block_end_index.is_none() {
        bail!("can't find name block for title: {}", title);
    }

    let name_block_end_index = maybe_name_block_end_index.unwrap();

    // name
    let name_block = blocks[1..=name_block_end_index].join(" ");
    // println!("{:?}, {:?}", name_block, maybe_name_block_end_index);

    // 处理name
    let (maybe_name_en, maybe_name_zh, maybe_name_jp) = name_process(&name_block);

    let tag_block_iter = blocks[name_block_end_index + 1..]
        .iter()
        .flat_map(|s| s.split(' '))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let (sub, dpi, source) = find_tags_from_iter(tag_block_iter);

    Ok(Episode::Ep(Ep {
        sub_group: group,
        season: maybe_season.unwrap_or(1),
        name_en: maybe_name_en,
        name_zh: maybe_name_zh,
        name_jp: maybe_name_jp,
        episode: maybe_ep.unwrap(),
        sub,
        resolution: dpi,
        source,
    }))
}

#[derive(Debug, PartialEq)]
pub enum Episode {
    Ep(Ep),
    Sp { name: String },
}

#[derive(Debug, PartialEq)]
pub struct Ep {
    sub_group: String,
    season: u8,
    pub name_en: Option<String>,
    name_zh: Option<String>,
    name_jp: Option<String>,
    pub episode: u32,
    sub: Option<String>,
    resolution: Option<String>,
    source: Option<String>,
}

impl Episode {
    pub fn unwrap_ep(self) -> Ep {
        match self {
            Episode::Ep(ep) => ep,
            _ => unreachable!("not a ep!"),
        }
    }

    pub fn name(&self, name_specific: Option<&str>) -> Result<String> {
        let mut name = None;
        if let Some(n) = name_specific {
            name = Some(n.to_string());
        } else {
            match self {
                Episode::Sp { name: sp_name } => {
                    name = Some(sp_name.clone());
                }
                Episode::Ep(ep) => {
                    if let Some(name_zh) = &ep.name_zh {
                        if name.is_none() {
                            name = Some(name_zh.clone());
                        }
                    }
                    if let Some(name_jp) = &ep.name_jp {
                        if name.is_none() {
                            name = Some(name_jp.clone());
                        }
                    }
                    if let Some(name_en) = &ep.name_en {
                        if name.is_none() {
                            name = Some(name_en.clone());
                        }
                    }
                }
            }
        }
        name.ok_or(eyre!("try to format path but all name is none"))
    }

    pub fn link_path(&self, name: &str) -> String {
        link_path(
            name,
            match self {
                Episode::Ep(ep) => &ep.season,
                Episode::Sp { .. } => &0,
            },
        )
    }

    pub fn link_file_name(&self, name: &str) -> String {
        match self {
            Episode::Ep(ep) => link_file_name(name, &ep.season, &ep.episode),
            Episode::Sp { name } => remove_video_ext_from(name),
        }
    }

    pub fn link_file_name_with_season(&self, name: &str, season: &u8) -> String {
        match self {
            Episode::Ep(ep) => link_file_name(name, season, &ep.episode),
            Episode::Sp { name } => remove_video_ext_from(name),
        }
    }
}

fn remove_video_ext_from(name: &str) -> String {
    for ext in VIDEO_EXTS.iter() {
        if name.to_lowercase().ends_with(&format!(".{ext}")) {
            return name[..name.len() - ext.len() - 1].to_string();
        }
    }
    name.to_string()
}

pub fn link_path(name: &str, season: &u8) -> String {
    format!("{}/Season {:0>2}", &name, &season)
}

pub fn link_file_name(name: &str, season: &u8, episode: &u32) -> String {
    format!("{} S{:0>2}E{}", &name, &season, &episode)
}

const SUBTITLE_EXTS: [&str; 2] = ["srt", "ass"];

pub fn link_external_subtitle(
    torrent: &Torrent,
    file_stem: &str,
    full_path: &str,
    link_file_name: &str,
    link_config: &config::Link,
    server_torrent: &dl::Torrent,
) -> Result<()> {
    let subtitle_reg = Regex::new(r"[._](.*)").unwrap();
    for file in torrent.files.as_ref().unwrap() {
        let file_suffix = file.path.extension().unwrap().to_str().unwrap();
        let file_name_from_torrent = file.path.file_name().unwrap().to_str().unwrap();
        if SUBTITLE_EXTS.iter().any(|ext| ext == &file_suffix) {
            let mut lan = None;
            if file_name_from_torrent.starts_with(file_stem) {
                let subtitle_block = file_name_from_torrent
                    .replace(file_stem, "")
                    .replace(&format!(".{file_suffix}"), "")
                    .to_lowercase();
                if let Some(c) = subtitle_reg
                    .captures(&subtitle_block)
                    .and_then(|c| c.get(1))
                {
                    lan = match c.as_str() {
                        "tc" | "zh-hant" => Some("zh-HK".to_string()),
                        "sc" | "zh-hans" | "" => Some("zh".to_string()),
                        "ja" => Some("ja".to_string()),
                        x => Some(x.to_string()),
                    };
                }
            } else {
                let zh_hints = ["简中", "簡中"];
                let zh_hk_hints = ["繁中"];
                let ja_hints = ["日文", "日语", "日語", "日本语", "日本語"];
                if zh_hints
                    .iter()
                    .any(|hint| file_name_from_torrent.contains(hint))
                {
                    lan = Some("zh".to_string());
                } else if zh_hk_hints
                    .iter()
                    .any(|hint| file_name_from_torrent.contains(hint))
                {
                    lan = Some("zh-HK".to_string());
                } else if ja_hints
                    .iter()
                    .any(|hint| file_name_from_torrent.contains(hint))
                {
                    lan = Some("ja".to_string());
                }
            }
            if let Some(lan) = lan {
                let link = format!("{full_path}/{link_file_name}.{lan}.{file_suffix}");
                if !path::Path::new(&link).exists() {
                    if link_config.dry_run {
                        println!(
                            "准备字幕链接{link} <- {}/{file_name_from_torrent}",
                            &torrent.name
                        );
                    } else {
                        std::fs::create_dir_all(full_path)?;
                        match std::fs::hard_link(
                            format!(
                                "{}/{}/{file_name_from_torrent}",
                                &server_torrent.download_dir, &torrent.name
                            ),
                            &link,
                        ) {
                            Ok(_) => {
                                println!(
                                    "创建字幕链接{link} <- {}/{file_name_from_torrent}",
                                    &torrent.name
                                );
                            }
                            Err(e) => println!("硬链接失败: {}", e),
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_video_ext_from() {
        let name = "[Up to 21°C] 擅长逃跑的殿下 / Nige Jouzu no Wakagimi - 9.5 (Baha 1920x1080 AVC AAC MP4)";
        assert_eq!(name, remove_video_ext_from(&name));
        assert_eq!(name, remove_video_ext_from(&format!("{name}.mp4")));
    }

    #[test]
    fn test_parser() {
        let ep = process("[Up to 21°C] 擅长逃跑的殿下 / Nige Jouzu no Wakagimi - 9.5 (Baha 1920x1080 AVC AAC MP4)");
        assert!(
            matches!(ep, Ok(Episode::Sp{ name }) if name == "[Up to 21°C] 擅长逃跑的殿下 / Nige Jouzu no Wakagimi - 9.5 (Baha 1920x1080 AVC AAC MP4)")
        );

        let ep = process("【幻樱字幕组】【4月新番】【古见同学有交流障碍症 第二季 Komi-san wa, Komyushou Desu. S02】【22】【GB_MP4】【1920X1080】");
        assert!(matches!(ep, Ok(Episode::Ep(_))));
        let ep = ep.unwrap().unwrap_ep();
        assert_eq!(ep.sub_group, "幻樱字幕组");
        assert_eq!(ep.season, 2);
        assert_eq!(ep.name_en, Some("Komi-san wa, Komyushou Desu.".to_string()));
        assert_eq!(ep.name_zh, Some("古见同学有交流障碍症".to_string()));
        assert_eq!(ep.episode, 22);
        assert_eq!(ep.sub, Some("GB".to_string()));
        assert_eq!(ep.resolution, Some("1920X1080".to_string()));

        let ep = process("[百冬练习组&LoliHouse] BanG Dream! 少女乐团派对！☆PICO FEVER！ / Garupa Pico: Fever! - 26 [WebRip 1080p HEVC-10bit AAC][简繁内封字幕][END] [101.69 MB]");
        assert!(matches!(ep, Ok(Episode::Ep(_))));
        let ep = ep.unwrap().unwrap_ep();
        assert_eq!(ep.sub_group, "百冬练习组&LoliHouse");
        assert_eq!(ep.season, 1);
        assert_eq!(ep.name_en, Some("Garupa Pico: Fever!".to_string()));
        assert_eq!(
            ep.name_zh,
            Some("BanG Dream! 少女乐团派对！☆PICO FEVER！".to_string())
        );
        assert_eq!(ep.episode, 26);
        assert_eq!(ep.sub, Some("简繁内封字幕".to_string()));
        assert_eq!(ep.resolution, Some("1080p".to_string()));
        assert_eq!(ep.source, Some("WebRip".to_string()));

        let ep =  process("【喵萌奶茶屋】★04月新番★[夏日重现/Summer Time Rendering][11][1080p][繁日双语][招募翻译]");
        assert!(matches!(ep, Ok(Episode::Ep(_))));
        let ep = ep.unwrap().unwrap_ep();
        assert_eq!(ep.sub_group, "喵萌奶茶屋");
        assert_eq!(ep.season, 1);
        assert_eq!(ep.name_en, Some("Summer Time Rendering".to_string()));
        assert_eq!(ep.name_zh, Some("夏日重现".to_string()));
        assert_eq!(ep.episode, 11);
        assert_eq!(ep.sub, Some("繁日双语".to_string()));
        assert_eq!(ep.resolution, Some("1080p".to_string()));

        let ep = process("[Lilith-Raws] 关于我在无意间被隔壁的天使变成废柴这件事 / Otonari no Tenshi-sama - 09 [Baha][WEB-DL][1080p][AVC AAC][CHT][MP4]");
        assert!(matches!(ep, Ok(Episode::Ep(_))));
        let ep = ep.unwrap().unwrap_ep();
        assert_eq!(ep.sub_group, "Lilith-Raws");
        assert_eq!(ep.season, 1);
        assert_eq!(ep.episode, 9);
        assert_eq!(ep.name_en, Some("Otonari no Tenshi-sama".to_string()));
        assert_eq!(ep.resolution, Some("1080p".to_string()));
        assert_eq!(
            ep.name_zh,
            Some("关于我在无意间被隔壁的天使变成废柴这件事".to_string())
        );

        let ep = process(
            "[梦蓝字幕组]New Doraemon 哆啦A梦新番[747][2023.02.25][AVC][1080P][GB_JP][MP4]",
        );
        assert!(matches!(ep, Ok(Episode::Ep(_))));
        let ep = ep.unwrap().unwrap_ep();
        assert_eq!(ep.sub_group, "梦蓝字幕组");
        assert_eq!(ep.season, 1);
        assert_eq!(ep.episode, 747);
        assert_eq!(ep.name_en, Some("New Doraemon".to_string()));
        assert_eq!(ep.resolution, Some("1080P".to_string()));
        assert_eq!(ep.name_zh, Some("哆啦A梦新番".to_string()));

        let ep = process(
            "[织梦字幕组][尼尔：机械纪元 NieR Automata Ver1.1a][02集][1080P][AVC][简日双语]",
        );
        assert!(matches!(ep, Ok(Episode::Ep(_))));
        let ep = ep.unwrap().unwrap_ep();
        assert_eq!(ep.sub_group, "织梦字幕组");
        assert_eq!(ep.season, 1);
        assert_eq!(ep.episode, 2);
        assert_eq!(ep.name_en, Some("NieR Automata Ver1.1a".to_string()));
        assert_eq!(ep.resolution, Some("1080P".to_string()));
        assert_eq!(ep.name_zh, Some("尼尔：机械纪元".to_string()));

        let ep = process(
            "[MagicStar] 假面骑士Geats / 仮面ライダーギーツ EP33 [WEBDL] [1080p] [TTFC]【生】",
        );
        assert!(matches!(ep, Ok(Episode::Ep(_))));
        let ep = ep.unwrap().unwrap_ep();
        assert_eq!(ep.sub_group, "MagicStar");
        assert_eq!(ep.season, 1);
        assert_eq!(ep.episode, 33);
        assert_eq!(ep.name_zh, Some("假面骑士Geats".to_string()));
        assert_eq!(ep.name_jp, Some("仮面ライダーギーツ".to_string()));
        assert_eq!(ep.resolution, Some("1080p".to_string()));

        let ep = process("【极影字幕社】★4月新番 天国大魔境 Tengoku Daimakyou 第05话 GB 720P MP4（字幕社招人内详）");
        assert!(matches!(ep, Ok(Episode::Ep(_))));
        let ep = ep.unwrap().unwrap_ep();
        assert_eq!(ep.sub_group, "极影字幕社");
        assert_eq!(ep.season, 1);
        assert_eq!(ep.episode, 5);
        assert_eq!(ep.name_zh, Some("天国大魔境".to_string()));
        assert_eq!(ep.name_en, Some("Tengoku Daimakyou".to_string()));
        assert_eq!(ep.resolution, Some("720P".to_string()));

        let ep = process("【极影字幕·毁片党】LoveLive! SunShine!! 幻日的夜羽 -SUNSHINE in the MIRROR- 第01集 TV版 HEVC_opus 1080p ");
        assert!(matches!(ep, Ok(Episode::Ep(_))));
        let ep = ep.unwrap().unwrap_ep();
        assert_eq!(ep.sub_group, "极影字幕·毁片党");
        assert_eq!(ep.season, 1);
        assert_eq!(ep.episode, 1);
        assert_eq!(ep.name_zh, Some("幻日的夜羽".to_string()));
        assert_eq!(ep.name_en, Some("LoveLive! SunShine!!".to_string()));
        assert_eq!(ep.resolution, Some("1080p".to_string()));

        let ep = process(
            "[ANi] BLEACH 死神 千年血战篇-诀别谭- - 14 [1080P][Baha][WEB-DL][AAC AVC][CHT][MP4]",
        );
        assert!(matches!(ep, Ok(Episode::Ep(_))));
        let ep = ep.unwrap().unwrap_ep();
        assert_eq!(ep.sub_group, "ANi");
        assert_eq!(ep.season, 1);
        assert_eq!(ep.episode, 14);
        assert_eq!(ep.name_zh, Some("死神 千年血战篇-诀别谭-".to_string()));
        assert_eq!(ep.name_en, Some("BLEACH".to_string()));
        assert_eq!(ep.resolution, Some("1080P".to_string()));
    }
}
