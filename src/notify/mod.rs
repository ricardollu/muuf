use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};

use crate::CLIENT;

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Notify {
    Ntfy { topic: String },
}

impl Notify {
    pub async fn link_success(&self, link_file_name: &str) -> Result<()> {
        match self {
            Notify::Ntfy { topic } => {
                CLIENT
                    .post(format!("https://ntfy.sh/{}", topic))
                    .body(format!("已下载<{link_file_name}>"))
                    .send()
                    .await?;
            }
        }
        Ok(())
    }
}
