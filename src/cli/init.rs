use crate::config::get_config;
use crate::template::{generate_lib_rs, generate_manifest};
use anyhow::{bail, Result};
use borderless_pkg::PkgType;
use cargo_toml_builder::prelude::*;
use cargo_toml_builder::types::CrateType;
use cliclack::log::error;
use cliclack::{confirm, select};
use cliclack::{input, intro, log::info, log::success};
use std::path::{Path, PathBuf};
use std::{env, fs};

fn validate_name(input: &String) -> Result<(), &'static str> {
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
}

/// Entrypoint for the `borderless init` subcommand
///
/// The input for this function is either:
/// - a name for the package that will be created ( `borderless init my-contract` )
/// - a directory, where the new package will be created ( `borderless init ./foo` )
/// - a reference to a github repo, that should serve as a template ( `borderless init @owner/repo:1.2.1` )
pub fn handle_init(name_or_path: Option<String>) -> Result<()> {
    intro("Initialize a new package 📦")?;
    let pkg_type = select("Please select the package type:")
        .item(PkgType::Contract, "Contract", "initializes a SmartContract")
        .item(PkgType::Agent, "Agent", "initializes a Software-Agent")
        .initial_value(PkgType::Contract)
        .interact()?;

    let (type_str, placeholder) = match pkg_type {
        PkgType::Contract => ("Contract", "my-contract"),
        PkgType::Agent => ("Agent", "my-agent"),
    };

    let name_or_path = name_or_path.unwrap_or(".".to_string());
    let try_path = PathBuf::from(name_or_path.clone());

    // If the given input is an existing path, we query for the name of the contract that should be created
    let (pkg_name, parent_dir) = if try_path.exists() {
        if !try_path.is_dir() {
            bail!("{} is not a directory", try_path.display());
        }
        let pkg_name = input(format!("{type_str} name"))
            .placeholder(placeholder)
            .validate(validate_name)
            .interact()?;
        (pkg_name, try_path)
    } else {
        // If it is not an existing path, we interpret it as the new package-name
        (name_or_path, env::current_dir()?)
    };

    let project_path = parent_dir.join(pkg_name.clone());

    // check the project path
    if project_path.exists() {
        bail!("Directory '{}' already exists", project_path.display());
    }

    if !confirm(format!(
        "Create project directory: {}",
        project_path.display()
    ))
    .interact()?
    {
        error("Process aborted by user")?;
        std::process::exit(1);
    }

    // create project path
    fs::create_dir_all(&project_path)?;

    info(format!(
        "Created project directory: {}",
        project_path.display()
    ))?;

    create_project_structure(&project_path, pkg_name, pkg_type)?;

    Ok(())
}

fn check_existence(path: &Path) -> Result<()> {
    if path.exists() {
        bail!(
            "'{}' already exists - refuse to overwrite existing project files",
            path.display()
        )
    }
    Ok(())
}

fn create_project_structure(
    project_path: &Path,
    pkg_name: String,
    pkg_type: PkgType,
) -> Result<()> {
    // src dir and basic files
    let src = project_path.join("src");
    let lib_file = src.join("lib.rs");
    let cargo_file = project_path.join("Cargo.toml");
    let manifest_file = project_path.join("Manifest.toml");

    // Sanity check
    check_existence(&src)?;
    check_existence(&lib_file)?;
    check_existence(&cargo_file)?;
    check_existence(&manifest_file)?;

    // Create src directory
    fs::create_dir_all(&src)?;

    // Get author
    let author = if let Some(author) = &get_config().author {
        author.to_string()
    } else {
        query_author()?
    };

    // Create Cargo.toml
    let cargo_toml_content = build_cargo_toml(&pkg_name)?;
    fs::write(&cargo_file, cargo_toml_content)?;

    // Create Manifest.toml
    let manifest = generate_manifest(&pkg_name, &pkg_type, vec![author.clone()])?;
    fs::write(&manifest_file, manifest)?;

    // Create src/lib.rs
    let lib_rs_content = generate_lib_rs(&pkg_name, &pkg_type)?;
    fs::write(&lib_file, lib_rs_content)?;

    success("Generated project files. Happy coding 💻!")?;
    Ok(())
}

fn build_cargo_toml(name: &str) -> Result<String> {
    let target = LibTarget::new().crate_type(CrateType::Cdylib).build();

    let cargo_toml = CargoToml::builder()
        // Package Section
        .name(name)
        .author("foo <foo@baa.com>")
        .version("0.1.0")
        .lib(target)
        .dependency(Dependency::branch(
            "borderless",
            "https://cargo-deploy-token:def035340885577ed9e9afeec98d8156678a7a74@git.borderless-technologies.com/Borderless/borderless.git",
            "main",
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
        .trim() // remove leading and trailing whitespace
        .to_string();

    // TODO only in verbose mode
    // info(format!("Generate project toml:\n{}", toml))?;
    Ok(toml)
}

/// Asks the user for the author
pub fn query_author() -> Result<String> {
    info("No author set in config. If you don't want to input these values, you can set the `author` field in your config.")?;
    let author: String = input("Author:")
        .placeholder("John Doe")
        .validate(|input: &String| {
            if input.trim().is_empty() {
                Err("Author cannot be empty")
            } else if !input
                .chars()
                .all(|c| c.is_alphabetic() || c.is_whitespace())
            {
                Err("Only letters allowed")
            } else if input.len() > 50 {
                Err("Contract name must be 50 characters or less")
            } else {
                Ok(())
            }
        })
        .interact()?;

    // Same as with author
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
    Ok(format!("{} <{}>", author, email))
}
