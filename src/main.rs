#![allow(clippy::collapsible_if)]
#![allow(clippy::comparison_chain)]

use std::process::exit;

use clap::Parser;

mod args;
mod cargo;
mod cmd;
mod config;
mod error;
mod git;
mod publish;
mod push;
mod release;
mod replace;
mod shell;
mod tag;
mod util;
mod version;

fn main() {
    let args::Command::Release(ref release_matches) = args::Command::parse();

    let mut builder = get_logging(release_matches.logging.log_level());
    builder.init();

    let res = match &release_matches.step {
        Some(args::Step::Config(config)) => config.run(),
        None => release::release_workspace(release_matches),
    };

    match res {
        Ok(code) => exit(code),
        Err(e) => {
            log::error!("Fatal: {}", e);
            exit(128);
        }
    }
}

pub fn get_logging(level: log::Level) -> env_logger::Builder {
    let mut builder = env_logger::Builder::new();

    builder.filter(None, level.to_level_filter());

    builder.format_timestamp_secs().format_module_path(false);

    builder
}
