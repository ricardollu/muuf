use color_eyre::eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let config = muuf::config::Config::load()?;
    if let Some(notify) = config.link.and_then(|link| link.notify) {
        notify.link_success("something for nothing s01e01").await?;
    }
    Ok(())
}
