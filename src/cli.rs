use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "borderless-cli")]
#[command(about = "borderless cli tools")]
#[command(version = "1.0")]
pub struct CommandLineInterface {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    New {
        project_name: String,
    },
    Pack {
        project_path: String,
        private_key: Option<String>,
    },
}
