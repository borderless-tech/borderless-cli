// use crate::packager::pack_wasm_contract;
use crate::template::{build_contract_manifest, generate_lib_rs, read_manifest};
use anyhow::{bail, Context, Result};
use cargo_toml_builder::prelude::*;
use cargo_toml_builder::types::CrateType;
use clap::{Parser, Subcommand};
use cliclack::{input, intro, log::info, log::success, spinner};
use regex;
use std::path::{Path, PathBuf};
use std::{env, fs};

// pub mod packager;
pub mod template;

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
        Commands::Init { project_name } => handle_new(project_name)?,
        Commands::Pack { project_path } => handle_pack(project_path, cli.private_key)?,
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

fn handle_new(name: String) -> Result<()> {
    intro("📜 Borderless Contract Creator")?;

    // build the project path
    let project_path = if name == "." {
        env::current_dir()?
    } else {
        let current_dir = env::current_dir()?;
        current_dir.join(&name)
    };

    // check the project path
    if name != "." && project_path.exists() {
        bail!("Directory '{}' already exists", name);
    }

    // create dir in case of subfolder
    if name != "." {
        fs::create_dir_all(&project_path)?;
        info(format!(
            "Created project directory: {}",
            project_path.display()
        ))?;
    } else {
        info(format!(
            "Using current directory: {}",
            project_path.display()
        ))?;
    }

    create_project_structure(&project_path)?;

    Ok(())
}

fn handle_pack(path: PathBuf, private_key: Option<String>) -> Result<()> {
    info("Create a smart contract bundle file!")?;
    info(format!("Read folder: {}", path.display()))?;

    let absolute_path = std::fs::canonicalize(&path).context("Failed to resolve absolute path")?;
    if !absolute_path.is_dir() {
        bail!("Not a directory: {}", absolute_path.display());
    }

    info(format!(
        "Working directory set to: {}",
        absolute_path.display()
    ))?;

    // check for manifest file
    check_manifest(&absolute_path)?;

    // read manifest
    let manifest = read_manifest(&absolute_path)?;

    // get private key
    let key_path = private_key
        .as_ref()
        .map(|s| {
            std::fs::canonicalize(s).with_context(|| format!("Private key file not found: {}", s))
        })
        .transpose()?;

    // build
    build_wasm(&absolute_path)?;

    // read wasm as bytes
    let wasm_bytes = read_wasm_file(&absolute_path, &manifest.contract.name)?;

    // pack contract
    // let bundle = pack_wasm_contract(&manifest, &wasm_bytes, key_path)?;

    // save_bundle_to_file(&bundle, &env::current_dir()?)?;
    success("Contract package created!")?;
    Ok(())
}

fn create_project_structure(project_name: &Path) -> Result<()> {
    // create src dir
    let src = project_name.join("src");

    if !src.exists() {
        fs::create_dir_all(&src)?;
    }

    // create project lib.rs
    let lib_file = src.join("lib.rs");

    // create Cargo.toml
    let cargo_file = project_name.join("Cargo.toml");

    // collect project information from user
    let contract_name: String = input("Contract name:")
        .placeholder("new-contract")
        .validate(|input: &String| {
            if input.trim().is_empty() {
                Err("Contract name cannot be empty")
            } else if !input
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                Err("Only letters, numbers, hyphens and underscores allowed")
            } else if input.len() > 50 {
                Err("Contract name must be 50 characters or less")
            } else {
                Ok(())
            }
        })
        .interact()?;

    let author: String = input("Author:")
        .placeholder("John Doe")
        .validate(|input: &String| {
            if input.trim().is_empty() {
                Err("Author cannot be empty")
            } else if !input.chars().all(|c| c.is_alphabetic()) {
                Err("Only letters allowed")
            } else if input.len() > 50 {
                Err("Contract name must be 50 characters or less")
            } else {
                Ok(())
            }
        })
        .interact()?;

    let email: String = input("Email:")
        .placeholder("john.doe@example.com")
        .validate(|input: &String| {
            let email = input.trim();
            if email.is_empty() {
                Err("Email cannot be empty")
            } else if !email.contains('@') {
                Err("Email must contain @")
            } else if !email.contains('.') {
                Err("Email must contain a domain")
            } else if email.len() > 254 {
                Err("Email must be 254 characters or less")
            } else if email.starts_with('@') || email.ends_with('@') {
                Err("Invalid email format")
            } else {
                Ok(())
            }
        })
        .interact()?;

    let cargo_toml_content = build_cargo_toml(&contract_name, &author, &email)?;
    std::fs::write(&cargo_file, cargo_toml_content)?;

    info("Generate contract manifest")?;
    let manifest_file = project_name.join("Manifest.toml");
    let manifest = build_contract_manifest(&contract_name, "0.1.0", "0.2.0")?;
    std::fs::write(&manifest_file, manifest)?;

    info("Generate Cargo settings!")?;
    let cargo_dir = project_name.join(".cargo");
    let config_file = cargo_dir.join("config.toml");
    fs::create_dir_all(&cargo_dir)?;
    let config_content = "[net]\ngit-fetch-with-cli = true\n";
    if config_file.exists() {
        let existing = fs::read_to_string(&config_file)?;
        if !existing.contains("git-fetch-with-cli = true") {
            fs::write(&config_file, format!("{}\n{}", existing, config_content))?;
            info("Added git-fetch-with-cli to existing ~/.cargo/config.toml")?;
        } else {
            info("git-fetch-with-cli already configured")?;
        }
    } else {
        fs::write(&config_file, config_content)?;
        info("Created ~/.cargo/config.toml with git-fetch-with-cli = true")?;
    }

    info("Generat project files!")?;
    let lib_rs_content = generate_lib_rs();
    std::fs::write(&lib_file, lib_rs_content)?;

    success("Successfully created smart contract project!")?;
    Ok(())
}

fn build_cargo_toml(name: &str, author: &str, email: &str) -> Result<String> {
    let target = LibTarget::new().crate_type(CrateType::Cdylib).build();

    let cargo_toml = CargoToml::builder()
        // Package Section
        .name(name)
        .author(&format!("{} <{}>", author, email))
        .version("0.1.0")
        .lib(target)
        .dependency(Dependency::tag(
            "borderless",
            "ssh://git@git.borderless-technologies.com:2222/Borderless/borderless.git",
            "v0.2.0",
        ))
        .dependency(Dependency::version("serde", "1.0"))
        .build()?;

    // toml postprocessing
    // fix edition
    let mut toml = cargo_toml.to_string();
    toml = toml.replace("[package]", "[package]\nedition = \"2021\"");

    // Fix crate-type format
    let re = regex::Regex::new(r#"crate[_-]type\s*=\s*"([^"]+)""#).unwrap();

    toml = re
        .replace_all(&toml, |caps: &regex::Captures| {
            let crate_type = &caps[1];
            format!(r#"crate-type = ["{}"]"#, crate_type)
        })
        .to_string();

    // TODO only in verbose mode
    info(format!("Generate project toml:\n{}", toml))?;
    Ok(toml)
}

fn check_manifest(work_dir: &Path) -> Result<()> {
    let manifest_path = work_dir.join("Manifest.toml");

    if !manifest_path.exists() {
        bail!("Manifest.toml not found in {}", work_dir.display());
    }

    info("Found Manifest.toml")?;
    Ok(())
}

fn build_wasm(work_dir: &std::path::Path) -> Result<()> {
    let sp = spinner();
    sp.start("Compiling to WebAssembly...");

    let child = std::process::Command::new("cargo")
        .args(&["build", "--release", "--target=wasm32-unknown-unknown"])
        .current_dir(work_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to start cargo build")?;

    // Während des Builds verschiedene Messages
    sp.set_message("Resolving dependencies...");

    let output = child
        .wait_with_output()
        .context("Failed to wait for cargo build")?;

    if output.status.success() {
        sp.stop("WASM build completed successfully!");

        // Optional: WASM files anzeigen
        let target_dir = work_dir.join("target/wasm32-unknown-unknown/release");
        if let Ok(entries) = std::fs::read_dir(&target_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("wasm") {
                    info(format!(
                        "Generated: {}",
                        entry.path().file_name().unwrap().to_string_lossy()
                    ))?;
                }
            }
        }
    } else {
        sp.stop("Build failed");
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("WASM build failed:\n{}", stderr);
    }

    Ok(())
}

fn read_wasm_file(work_dir: &Path, contract_name: &str) -> Result<Vec<u8>> {
    // WASM-File Pfad ermitteln
    let wasm_path = work_dir
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("release")
        .join(format!("{}.wasm", contract_name));

    // Prüfen ob File existiert
    if !wasm_path.exists() {
        bail!("WASM file not found: {}", wasm_path.display());
    }

    // Als Vec<u8> einlesen
    let wasm_bytes = std::fs::read(&wasm_path)
        .with_context(|| format!("Failed to read WASM file: {}", wasm_path.display()))?;

    info(format!(
        "Read WASM file: {} ({} bytes)",
        wasm_path.display(),
        wasm_bytes.len()
    ))?;

    Ok(wasm_bytes)
}

// fn save_bundle_to_file(bundle: &Bundle, path: &Path) -> Result<()> {
//     let json = serde_json::to_string_pretty(bundle)?;
//     std::fs::write(path.join("package.json"), json)?;
//     info(format!("Bundle saved to {}", path.display()))?;
//     Ok(())
// }
