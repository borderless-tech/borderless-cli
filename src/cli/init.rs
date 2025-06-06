use crate::config::get_config;
use crate::template::{generate_lib_rs, generate_manifest};
use anyhow::{bail, Result};
use borderless_pkg::PkgType;
use cliclack::{confirm, select};
use cliclack::{input, intro, log::info, log::success};
use std::path::{Path, PathBuf};
use std::{env, fs};

#[allow(clippy::ptr_arg)]
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
        .item(
            PkgType::Contract,
            "Contract 🔗  ",
            "initializes a SmartContract",
        )
        .item(
            PkgType::Agent,
            "Agent    🤖✨",
            "initializes a Software-Agent",
        )
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
        // If it is not an existing path, it could be a path we should create.
        //
        // The last segment of the path is in that case the package name (if it is only one segment, that is the package name).
        let as_path = PathBuf::from(&name_or_path);
        match as_path.file_name() {
            Some(name) => {
                let name = name.to_string_lossy().to_string();
                let current_dir = env::current_dir()?;
                let parent = as_path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or(current_dir);
                (name, parent)
            }
            None => (name_or_path, env::current_dir()?),
        }
    };

    let project_path = parent_dir.join(pkg_name.clone());

    // check the project path
    if project_path.exists() {
        bail!("Directory '{}' already exists", project_path.display());
    }

    if get_config().confirm_creation
        && !confirm(format!(
            "Create project directory: {}",
            project_path.display()
        ))
        .interact()?
    {
        bail!("Process aborted by user.");
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
    let cargo_toml_content = build_cargo_toml(&pkg_name, &author)?;
    fs::write(&cargo_file, cargo_toml_content)?;

    // Create Manifest.toml
    let manifest = generate_manifest(&pkg_name, &pkg_type, vec![author])?;
    fs::write(&manifest_file, manifest)?;

    // Create src/lib.rs
    let lib_rs_content = generate_lib_rs(&pkg_name, &pkg_type)?;
    fs::write(&lib_file, lib_rs_content)?;

    success("Generated project files. Happy coding 💻!")?;
    Ok(())
}

fn build_cargo_toml(name: &str, author: &str) -> Result<String> {
    use cargo_toml::*;

    // Build package ( since we don't use the metadata section, we set the generic type to unit '()' )
    let mut package: Package<()> = Package::default();
    package.name = name.to_string();
    package.version = Inheritable::Set("0.1.0".to_string());
    package.edition = Inheritable::Set(Edition::E2021);
    package.authors = Inheritable::Set(vec![author.to_string()]);

    // Specify dependencies
    let mut dependencies = DepsSet::new();
    dependencies.insert("serde".to_string(), Dependency::Simple("1.0".to_string()));
    let borderless = DependencyDetail {
        git: Some("https://cargo-deploy-token:def035340885577ed9e9afeec98d8156678a7a74@git.borderless-technologies.com/Borderless/borderless.git".to_string()),
        branch: Some("main".to_string()),
        ..Default::default()
    };
    dependencies.insert(
        "borderless".to_string(),
        Dependency::Detailed(Box::new(borderless)),
    );

    // Set crate type to "cdylib" (necessary for wasm)
    let lib = Product {
        crate_type: vec!["cdylib".to_string()],
        ..Default::default()
    };

    // Set release profile to optimize for binary size
    let profile = Profiles {
        release: Some(Profile {
            opt_level: Some(toml::Value::String("z".to_string())),
            lto: Some(LtoSetting::Fat),
            codegen_units: Some(1),
            debug: None,
            split_debuginfo: None,
            rpath: None,
            debug_assertions: None,
            panic: None,
            incremental: None,
            overflow_checks: None,
            strip: None,
            package: Default::default(),
            build_override: None,
            inherits: None,
        }),
        ..Default::default()
    };

    let cargo = Manifest {
        package: Some(package),
        dependencies,
        lib: Some(lib),
        profile,
        ..Default::default()
    };

    let toml = toml::to_string(&cargo)?.replace("required-features = []\n", "");
    Ok(toml)
}

/// Asks the user for the author
pub fn query_author() -> Result<String> {
    info("Please tell us who you are. If you don't want to input these values everytime, you can set the `author` field in your config.")?;
    let author: String = input("Name:")
        .placeholder("John Doe")
        .validate(|input: &String| {
            if input.trim().is_empty() {
                Err("Name cannot be empty")
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
