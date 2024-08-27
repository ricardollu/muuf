use color_eyre::eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!(
        "{}",
        toml::to_string(&muuf::config::Config::load().unwrap()).unwrap()
    );

    Ok(())
}
