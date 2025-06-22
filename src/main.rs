// use crate::packager::pack_wasm_contract;
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use cliclack::log::error;
use std::{fs, path::PathBuf};

// pub mod packager;
mod template;

mod cli;

mod api;

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
    Init { project_name: Option<String> },

    /// Creates a new package from an existing project
    Pack { project_path: PathBuf },

    /// Merges an introduction with a package.json
    Merge {
        introduction: PathBuf,
        package_json: PathBuf,
    },

    /// Deploys a package to a node
    Deploy { path: PathBuf },

    /// Links the cli to a node or registry
    ///
    /// This makes the node or registry available for commands like `publish` or `deploy`
    Link,

    /// Publishes a package to some registry
    Publish,

    /// Create a new template
    #[command(subcommand)]
    Template(TemplateCmd),
}

#[derive(Subcommand)]
pub enum TemplateCmd {
    Introduction,
}

fn main() -> Result<()> {
    // Register config object
    config::init_config()?;

    // Check that data directory exists
    let data_dir = config::get_config()
        .data_dir()
        .context("failed to get data directory - consider setting it manually in your config")?;

    if !data_dir.exists() {
        fs::create_dir_all(&data_dir)?;
    }

    if !data_dir.is_dir() {
        bail!("data-directory {} is not a directory!", data_dir.display());
    }

    // Parse arguments
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Init { project_name } => cli::handle_init(project_name),
        Commands::Pack { project_path } => cli::handle_pack(project_path),
        Commands::Merge {
            introduction,
            package_json,
        } => cli::handle_merge(introduction, package_json),
        Commands::Deploy { path } => cli::handle_deploy(path),
        Commands::Link => cli::handle_link(),
        Commands::Publish => todo!(),
        Commands::Template(template) => cli::handle_template(template),
    };

    if let Err(e) = result {
        error(format!("{e}"))?;
    }

    Ok(())
}

mod config {
    use anyhow::{Context, Result};
    use borderless_pkg::Author;
    use once_cell::sync::OnceCell;
    use serde::{Deserialize, Serialize};
    use std::env;
    use std::fs::read_to_string;
    use std::path::PathBuf;

    /// Name of the config file
    const CONFIG_FILE_NAME: &str = "config.toml";

    /// Name of the specific config directory for our config
    const CONFIG_DIR_NAME: &str = "borderless-cli";

    pub static CONFIG: OnceCell<Config> = OnceCell::new();

    /// Configuration of the cmdline interface
    #[derive(Debug, Default, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct Config {
        /// Author information
        pub author: Option<Author>,

        /// If true, the user has to confirm the creation of new directories
        pub confirm_creation: bool,

        /// Base data directory.
        ///
        /// Defaults to `XDG_DATA_HOME`
        data_directory: Option<PathBuf>,
    }

    impl Config {
        pub fn data_dir(&self) -> Result<PathBuf> {
            match &self.data_directory {
                Some(dir) => Ok(dir.clone()),
                None => {
                    let base_dir: PathBuf = env::var("XDG_DATA_HOME")?.into();
                    Ok(base_dir.join("borderless-cli"))
                }
            }
        }
    }

    fn get_config_file_dir() -> Option<PathBuf> {
        // use XGD_CONFIG_HOME if possible, otherwise $HOME/.config
        let base_dir = if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg_config_home)
        } else {
            let home = env::var("HOME").ok()?;
            PathBuf::from(home).join(".config")
        };

        if !base_dir.exists() {
            return None;
        }

        // check XDG_CONFIG_DIRS for system wide configs
        let config_dirs = env::var("XDG_CONFIG_DIRS").unwrap_or_else(|_| "/etc/xdg".to_string());

        for dir in config_dirs.split(":") {
            if dir.trim().is_empty() {
                continue;
            }

            let config_file = PathBuf::from(dir)
                .join(CONFIG_DIR_NAME)
                .join(CONFIG_FILE_NAME);

            if config_file.exists() {
                return Some(config_file);
            }
        }

        Some(base_dir)
    }

    fn get_config_file_path() -> Option<PathBuf> {
        let config_file_path = get_config_file_dir()?
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME);

        Some(config_file_path)
    }

    /// Initializes the config
    ///
    /// This registers the static, global variable `CONFIG`, which can be easily accessed via [`get_config()`]
    pub fn init_config() -> Result<()> {
        let config = match config_file() {
            Some(file) => {
                // Read config from disk
                let content = read_to_string(file)?;
                toml::from_str(&content)?
            }
            None => {
                let default_config = Config::default();
                let config_file_path = get_config_file_path()
                    .context("failed to get config directory - consider to set XDG_CONFIG_HOME.")?;

                if let Some(parent) = config_file_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                let config_content = toml::to_string_pretty(&default_config)?;
                std::fs::write(&config_file_path, config_content)?;

                println!("Initial setup complete! We've created a configuration file at:");
                println!("  {}", config_file_path.display());
                println!("You can customize the settings by editing this file.");

                default_config
            }
        };

        CONFIG.set(config).expect("config is unset");
        Ok(())
    }

    /// Returns a reference to the current config object
    pub fn get_config() -> &'static Config {
        CONFIG.get().expect("config has not been initialized")
    }

    fn config_file() -> Option<PathBuf> {
        let config_file = get_config_file_path()?;

        // check if config file exists
        if config_file.exists() {
            return Some(config_file);
        }

        None
    }
}
