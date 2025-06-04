// use crate::packager::pack_wasm_contract;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

// pub mod packager;
pub mod template;

mod cli;

#[derive(Parser)]
#[command(name = "borderless")]
#[command(about = "borderless cmdline tool")]
pub struct Cli {
    /// Override the private key that should be used for signing
    #[arg(long)]
    private_key: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

/*
 * Let's flesh out what we want here on a high-level:
 *
 * Things we definetely want
 * borderless init -> create new projects, similar to cargo init; maybe with the use of templates like npm allows it (borderless init @foo/baa)
 * borderless pack PATH -> create a package from an existing project
 * borderless deploy -> deploy a package to some node
 * borderless link -> link the CLI to some external node or registry
 * borderless publish -> publish a package to a registry
 *
 * Not so fleshed out ideas:
 * borderless info -> show linked devices, available private-keys, etc.
 * borderless config -> shows info about the cmdline tool in general (and ability to create default config)
 *
 * borderless run -> multiple sub-commands (?)
 *  borderless run dev -> similar to npm run dev (big question: how do we handle the initial state etc. ?)
 *
 * General things:
 * - the tool should be configurable with a normal config file that lives under $XDG_CONFIG_HOME/borderless-cli
 * - if no config file exists, it should use a default config
 * - the tool should use a default data directory for persistent data (like e.g. private keys or to remember linked nodes),
 * - the data directory should default to $XDG_DATA_HOME/borderless-cli
 * - it would be cool to compile this with musl, so we can get a self-contained binary (or get close to a self-contained binary)
 *
 */

#[derive(Subcommand)]
pub enum Commands {
    /// Initializes a new project
    Init { project_name: String },

    /// Creates a new package from an existing project
    Pack { project_path: PathBuf },

    /// Deploys a package to a node
    Deploy,

    /// Links the cli to a node or registry
    ///
    /// This makes the node or registry available for commands like `publish` or `deploy`
    Link,

    /// Publishes a package to some registry
    Publish,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Get configuration
    let _config = config::get_config()?;

    // Parse commands
    match cli.command {
        Commands::Init { project_name } => cli::handle_init(project_name)?,
        Commands::Pack { project_path } => cli::handle_pack(project_path)?,
        Commands::Deploy => todo!(),
        Commands::Link => todo!(),
        Commands::Publish => todo!(),
    }

    Ok(())
}

mod config {
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use std::env;
    use std::fs::read_to_string;
    use std::path::PathBuf;

    /// Name of the config file
    const CONFIG_FILE_NAME: &str = "config.toml";

    /// Name of the specific config directory for our config
    const CONFIG_DIR_NAME: &str = "borderless-cli";

    /// Configuration of the cmdline interface
    #[derive(Default, Serialize, Deserialize)]
    pub struct Config {}

    /// Retrieves the configuration
    pub fn get_config() -> Result<Config> {
        match config_file() {
            Some(file) => {
                // Read config from disk
                let content = read_to_string(file)?;
                // NOTE: We could also just use the default config, if something fails
                let config: Config = toml::from_str(&content)?;
                Ok(config)
            }
            None => Ok(Config::default()),
        }
    }

    fn config_file() -> Option<PathBuf> {
        // TODO: Search XDG_CONFIG_DIRS in case is is not found at XDG_CONFIG_HOME
        let base_dir: PathBuf = env::var("XDG_CONFIG_HOME").ok()?.into();
        let config_dir = base_dir.join(CONFIG_DIR_NAME);
        if !config_dir.exists() {
            return None;
        }
        let config_file = config_dir.join(CONFIG_FILE_NAME);
        if !config_file.exists() {
            return None;
        }
        Some(config_file)
    }
}
