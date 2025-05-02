use std::{fs, path::Path};

use anyhow::Result;
use config::{Config, File};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Encrypt {
    pub private_key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Admin {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Conf {
    /// if enable debug mode
    pub debug: bool,
    pub bind_addr: String,
    // api url debug
    pub api_url: String,
    pub redis_url: String,
    pub encrypt: Encrypt,
    pub comet_secret: String,
    pub database_url: String,
    pub admin: Admin,
    #[serde(skip)]
    config_file: String,
}

impl Conf {
    pub fn get_config_file(&self) -> String {
        self.config_file.to_owned()
    }
}

impl Conf {
    pub fn parse(filename: &str) -> Result<Self> {
        let v = Config::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name(filename).required(false))
            .build_cloned()?
            .try_deserialize()?;
        Ok(v)
    }

    pub fn sync2file(&self, filepath: Option<String>) -> Result<()> {
        let toml = toml::to_string_pretty(self)?;
        let filepath = if let Some(v) = filepath {
            v
        } else {
            "jiascheduler-console.toml".to_string()
        };
        let real_path = shellexpand::full(&filepath)?.to_string();

        if let Some(p) = Path::new(&real_path).parent() {
            if !p.exists() {
                fs::create_dir_all(p)?;
            }
        }
        let ret = fs::write(&real_path, toml)?;
        Ok(ret)
    }
}
