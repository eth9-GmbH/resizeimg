#![forbid(unsafe_code)]
use crate::image::EngineType;
use serde::Deserialize;
use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::path::Path;

const ENV_CONFIG_VAR: &str = "RESIZEIMG_CFG";

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Upstreams {
    pub path: String,
    pub upstream: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Config {
    pub port: Option<u16>,
    pub listen_address: Option<String>,
    pub threads: Option<usize>,
    pub engine: EngineType,
    pub upstreams: Vec<Upstreams>,
}
impl Config {
    pub fn read(path: &Option<OsString>) -> anyhow::Result<Self> {
        let mut buffer = String::new();
        if let Some(cfg_path) = path {
            info!("Reading configuration from file: '{:?}'", cfg_path);
            let mut configfile = File::open(Path::new(&cfg_path))?;
            configfile.read_to_string(&mut buffer)?;
        } else {
            info!("Reading configuration from env variable {ENV_CONFIG_VAR}");
            buffer = env::var(ENV_CONFIG_VAR)?;
        }
        let config = toml::from_str::<Config>(&buffer)?;
        trace!("Config: {:#?}", config);
        Ok(config)
    }
}
