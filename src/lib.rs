pub mod cli;
pub mod config;

pub fn run() -> Result<(), clap::Error> {
    let _config = cli::parse_config_from_env()?;
    Ok(())
}
