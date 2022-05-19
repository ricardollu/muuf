use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use muuf::{dl::Client, res::ApiServer, *};
use owo_colors::OwoColorize;
use tabled::{builder::Builder, object::Columns, Format, Modify, Style};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.commands {
        Commands::Res(c) => {
            let api = res::get_res_api(&CONFIG.res_api);
            match c.commands {
                ResCommands::Search(cc) => {
                    let (list, has_more) = api.res_list(&cc.keywords, cc.subgroup_id, cc.res_type_id).await?;
                    print_res_list(list, has_more);
                }
                ResCommands::Type => print_id_and_name(api.res_types().await?, "分类#ID", 5),
                ResCommands::SubGroup => print_id_and_name(api.sub_groups().await?, "字幕组#ID", 5),
                ResCommands::Calendar => todo!(),
            }
        }
        Commands::Watch => watch().await?,
        Commands::Check => check().await?,
    }

    Ok(())
}

async fn watch() -> Result<()> {
    let interval = CONFIG.interval; // make sure CONFIG is loaded ahead
    loop {
        check().await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
    }
}

async fn check() -> Result<()> {
    let dl_client = dl::get_client(&CONFIG.downloader);
    let dl_server_torrents = dl_client.torrent_get().await?;
    println!("{} rules to be checked", CONFIG.rules.len());
    for rule in CONFIG.rules.iter() {
        let res_api = res::get_res_api(&rule.res_api);
        let (res_list, _) = res_api.res_list(&rule.keywords, rule.sub_group_id, rule.res_type_id).await?;
        for res in res_list {
            if dl_server_torrents.iter().any(|t| compare_res_to_torrent(&res, t)) {
                // println!("{} already in download server", res.title);
                continue;
            }
            dl_client.torrent_add(res.magnet.to_string(), &rule.name).await?;
            println!("加入下载列表: {}", res.title)
        }
    }

    Ok(())
}

fn compare_res_to_torrent(res: &res::Res, torrent: &dl::Torrent) -> bool {
    res.info_hash == torrent.hash
}

fn print_id_and_name(data: Vec<(i32, String)>, title: &str, chunk_size: usize) {
    let mut table_builder = Builder::default().set_columns([title]);
    for chunk in data.chunks(chunk_size) {
        table_builder = table_builder.add_record(chunk.iter().map(|(id, name)| format!("{name}#{id}")).collect::<Vec<_>>());
    }
    println!(
        "{}",
        table_builder
            .build()
            .with(Style::github_markdown())
            .with(Modify::new(Columns::new(0..)).with(Format::new(|s| s.bright_green().to_string()))),
    );
}

fn print_res_list(list: Vec<res::Res>, has_more: bool) {
    let mut table_builder = Builder::default().set_columns(["发布时间", "标题", "类型", "字幕组", "大小"]);
    for res in list {
        table_builder = table_builder.add_record(vec![
            res.publish_date,
            res.title,
            format!("{}#{}", res.type_name, res.type_id),
            if res.sub_group_id > 0 {
                format!("{}#{}", res.sub_group_name, res.sub_group_id)
            } else {
                res.sub_group_name
            },
            res.file_size,
        ]);
    }
    println!(
        "{}{}",
        table_builder
            .build()
            .with(Style::github_markdown())
            .with(Modify::new(Columns::new(0..)).with(Format::new(|s| s.bright_green().to_string()))),
        if has_more {
            "还有更多...".bright_yellow().to_string()
        } else {
            "".to_string()
        }
    );
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 资源相关子命令
    Res(Res),
    /// 持续运行，间隔{interval}时间检查
    Watch,
    /// 检查一次
    Check,
}

#[derive(Args)]
struct Res {
    #[clap(subcommand)]
    commands: ResCommands,
}

#[derive(Subcommand)]
enum ResCommands {
    /// 搜索 [关键字] -t=类型ID -s=字幕组ID
    Search(ResListCommand),
    /// 资源类型
    Type,
    /// 字幕组
    SubGroup,
    /// todo 新番日历
    Calendar,
}

#[derive(Args)]
struct ResListCommand {
    /// 资源类型ID
    #[clap(short = 't', long)]
    res_type_id: Option<i32>,
    /// 字幕组ID
    #[clap(short, long)]
    subgroup_id: Option<i32>,
    /// 搜索关键字
    keywords: Vec<String>,
}
