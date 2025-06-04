use crate::template::{build_contract_manifest, generate_lib_rs};
use anyhow::{bail, Result};
use cargo_toml_builder::prelude::*;
use cargo_toml_builder::types::CrateType;
use cliclack::{input, intro, log::info, log::success};
use regex;
use std::path::Path;
use std::{env, fs};

pub fn handle_init(name: String) -> Result<()> {
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

fn create_project_structure(project_path: &Path) -> Result<()> {
    // create src dir
    let src = project_path.join("src");

    if !src.exists() {
        fs::create_dir_all(&src)?;
    }

    // create project lib.rs
    let lib_file = src.join("lib.rs");

    // create Cargo.toml
    let cargo_file = project_path.join("Cargo.toml");

    // collect project information from user
    //
    // TODO: Select contract or agent here
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
    let manifest_file = project_path.join("Manifest.toml");
    let manifest = build_contract_manifest(&contract_name, "0.1.0", "0.2.0")?;
    std::fs::write(&manifest_file, manifest)?;

    info("Generate Cargo settings!")?;
    let cargo_dir = project_path.join(".cargo");
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
