#![allow(clippy::collapsible_if)]
#![allow(clippy::comparison_chain)]

use clap::Parser;

mod args;
mod config;
mod error;
mod ops;
mod steps;

fn main() {
    let res = run();
    error::exit(res)
}

fn run() -> Result<(), error::ProcessError> {
    let args::Command::Release(ref release_matches) = args::Command::parse();

    let mut builder = get_logging(release_matches.logging.log_level());
    builder.init();

    if let Some(dump_config) = release_matches.dump_config.as_deref() {
        log::trace!("Initializing");
        let ws_meta = release_matches
            .manifest
            .metadata()
            // When evaluating dependency ordering, we need to consider optional depednencies
            .features(cargo_metadata::CargoOpt::AllFeatures)
            .exec()
            .map_err(error::FatalError::from)?;

        let release_config =
            if let Some(root_id) = ws_meta.resolve.as_ref().and_then(|r| r.root.as_ref()) {
                let pkg = ws_meta
                    .packages
                    .iter()
                    .find(|p| p.id == *root_id)
                    .expect("root should always be present");

                let mut release_config = config::Config::from_defaults();
                release_config.update(&config::load_package_config(
                    &release_matches.config,
                    &ws_meta,
                    pkg,
                )?);
                release_config
            } else {
                let mut release_config = config::Config::from_defaults();
                release_config.update(&config::load_workspace_config(
                    &release_matches.config,
                    &ws_meta,
                )?);
                release_config
            };

        let output = toml_edit::easy::to_string_pretty(&release_config)?;

        if dump_config == std::path::Path::new("-") {
            use std::io::Write;
            std::io::stdout().write_all(output.as_bytes())?;
        } else {
            std::fs::write(dump_config, &output)?;
        }

        Ok(())
    } else {
        match &release_matches.step {
            Some(args::Step::Version(config)) => config.run(),
            Some(args::Step::Replace(config)) => config.run(),
            Some(args::Step::Publish(config)) => config.run(),
            Some(args::Step::Tag(config)) => config.run(),
            Some(args::Step::Push(config)) => config.run(),
            Some(args::Step::Config(config)) => config.run(),
            None => steps::release::release_workspace(release_matches),
        }
    }
}

pub fn get_logging(level: log::Level) -> env_logger::Builder {
    let mut builder = env_logger::Builder::new();

    builder.filter(None, level.to_level_filter());

    builder.format_timestamp_secs().format_module_path(false);

    builder
}
