pub mod config;
pub mod dl;
pub mod res;

use once_cell::sync::Lazy;

pub static CONFIG: Lazy<config::Config> = Lazy::new(|| config::load().unwrap());
pub static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    let mut client_builder = reqwest::Client::builder();
    if let Some(config) = &CONFIG.proxy {
        let proxy = reqwest::Proxy::all(config.scheme.to_string()).unwrap();
        client_builder = client_builder.proxy(proxy);
    }
    client_builder.build().unwrap()
});
