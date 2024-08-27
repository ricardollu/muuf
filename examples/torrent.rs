use color_eyre::eyre::Result;
use muuf::parser;

#[tokio::main]
async fn main() -> Result<()> {
    // let page_url = "https://mikanani.me/Home/Episode/86c17c70e59313f891c9a89feef17fdefd7fe34c";
    let url =
        // "https://mikanani.me/Download/20151111/86c17c70e59313f891c9a89feef17fdefd7fe34c.torrent";
    "https://dl.dmhy.org/2023/08/30/14fc0eedf87018f8fbcc05bbbf5573a94fb64239.torrent";

    let torrent =
        lava_torrent::torrent::v1::Torrent::read_from_bytes(muuf::get_url_bytes(url).await?)?;

    println!("{:?}", torrent.name);
    torrent.files.unwrap().iter().for_each(|file| {
        let maybe_extension = file.path.extension().and_then(|s| s.to_str());
        if maybe_extension == Some("mkv") || maybe_extension == Some("mp4") {
            println!("parent: {:?}", file.path.parent());
            println!("{}", file.path.file_name().unwrap().to_str().unwrap());
            println!(
                "{:?}",
                parser::process(file.path.file_name().unwrap().to_str().unwrap()) // file.path.to_str().unwrap()
                    .map(|ep| (ep.name_en, ep.episode))
            );
        }
    });

    // torrent.files.unwrap().iter().for_each(|file| {
    //     println!("{}", file);
    // });
    Ok(())
}
