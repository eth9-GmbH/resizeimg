#![forbid(unsafe_code)]
#[macro_use]
extern crate log;

mod config;
mod http;
mod image;

use crate::config::Config;
use crate::http::http_server;
use crate::image::EngineType;

use anyhow::Result;
use clap::Parser;
use env_logger::Env;
use libvips::VipsApp;
use std::ffi::OsString;

#[derive(Parser)]
struct CmdOpts {
    #[arg(long, short)]
    config_file: Option<OsString>,
}

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();
    let cmdopts = CmdOpts::parse();
    match run(&cmdopts) {
        Ok(()) => {
            std::process::exit(0);
        }
        Err(e) => {
            error!("{e}");
            std::process::exit(1);
        }
    };
}

fn run(cmdopts: &CmdOpts) -> Result<()> {
    let config = Config::read(&cmdopts.config_file)?;
    if config.engine == EngineType::Vips {
        let vips: VipsApp = VipsApp::new(env!("CARGO_PKG_NAME"), false)?;
        vips.concurrency_set(2);
        vips.cache_set_max(0);
        vips.cache_set_max_mem(0);
    }
    let runtime = match config.threads {
        None => tokio::runtime::Builder::new_multi_thread().enable_all().build()?,
        Some(0) | Some(1) => tokio::runtime::Builder::new_current_thread().enable_all().build()?,
        Some(n) => {
            tokio::runtime::Builder::new_multi_thread().worker_threads(n).enable_all().build()?
        }
    };
    runtime.block_on(http_server(config))?;
    Ok(())
}
