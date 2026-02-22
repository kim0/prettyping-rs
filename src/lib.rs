pub mod app;
pub mod cli;
pub mod config;
pub mod engine;
pub mod net;

pub fn run() -> Result<(), clap::Error> {
    let _config = cli::parse_config_from_env()?;
    Ok(())
}
