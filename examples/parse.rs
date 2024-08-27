use color_eyre::eyre::Result;
use muuf::{
    parser,
    res::{self, ApiServer, Res},
};

#[tokio::main]
async fn main() -> Result<()> {
    let config = muuf::config::Config::load()?;
    let api = res::get_res_api(&config.res_api);
    let (list, _) = api.res_list(&[], None, Some(2), None).await?;
    for Res { title, .. } in list {
        println!("{}", title);
        println!("{:?}", parser::process(&title))
    }

    Ok(())
}
