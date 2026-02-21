use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "prettyping-rs", about = "Rust port of prettyping")]
pub struct CliArgs {
    #[arg(value_name = "HOST")]
    pub host: Option<String>,
}
