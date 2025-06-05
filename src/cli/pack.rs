use anyhow::{bail, Context, Result};
use borderless_hash::Hash256;
use borderless_pkg::*;
use cliclack::{
    intro,
    log::{info, success},
    spinner,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::template::Manifest;

pub fn handle_pack(path: PathBuf) -> Result<()> {
    let absolute_path = fs::canonicalize(&path).context("Failed to resolve absolute path")?;
    if !absolute_path.is_dir() {
        bail!("Not a directory: {}", absolute_path.display());
    }

    // Validate the project directory
    check_project_structure(&path)?;

    // Parse the manifest
    let manifest = read_manifest(&path).context("failed to read Manifest.toml")?;
    let (pkg_type, pkg_info) = match (manifest.agent, manifest.contract) {
        (Some(info), None) => {
            intro(format!("📦 Create package for agent '{}'", info.name))?;
            (PkgType::Agent, info)
        }
        (None, Some(info)) => {
            intro(format!("📦 Create package for contract '{}'", info.name))?;
            (PkgType::Contract, info)
        }
        _ => bail!("invalid manifest - either [agent] or [contract] section must be set"),
    };

    // Also read cargo.toml to get the version
    let version = get_version_from_cargo(&path)?;

    info(format!("Read folder: {}", path.display()))?;

    info(format!(
        "Working directory set to: {}",
        absolute_path.display()
    ))?;

    // Compile the project
    compile_project(&absolute_path)?;

    // read wasm as bytes
    let wasm_bytes = read_wasm_file(&absolute_path, &pkg_info.name)?;

    // Create package
    let pkg = WasmPkg {
        name: pkg_info.name.clone(),
        app_name: pkg_info.app_name,
        app_module: pkg_info.app_module,
        capabilities: manifest.capabilities,
        pkg_type,
        meta: manifest.meta.unwrap_or_default(),
        source: Source {
            version,
            digest: Hash256::digest(&wasm_bytes),
            code: SourceType::Wasm {
                wasm: wasm_bytes,
                git_info: None,
            },
        },
    };

    // pack contract
    // let bundle = pack_wasm_contract(&manifest, &wasm_bytes, key_path)?;

    // save_bundle_to_file(&bundle, &env::current_dir()?)?;
    success(format!("Successfully packaged '{}'", pkg_info.name))?;
    Ok(())
}

/// Validate the project structure
fn check_project_structure(path: &Path) -> Result<()> {
    let cargo = path.join("Cargo.toml");
    let src = path.join("src");
    let lib = src.join("lib.rs");
    let manifest = path.join("Manifest.toml");
    let must_exist = [cargo, src, lib, manifest];
    for p in must_exist {
        if !p.exists() {
            bail!("missing {} in project directory", p.display());
        }
    }
    Ok(())
}

/// Read the manifest from the project dir
fn read_manifest(project_dir: &Path) -> Result<Manifest> {
    let manifest_path = project_dir.join("Manifest.toml");
    let content = fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = toml::from_str(&content)?;
    Ok(manifest)
}

fn get_version_from_cargo(path: &Path) -> Result<SemVer> {
    let manifest_path = path.join("Cargo.toml");
    let content = fs::read_to_string(&manifest_path)?;
    let manifest: cargo_toml::Manifest = toml::from_str(&content)?;
    Ok(manifest
        .package
        .context("missing [package] section in Cargo.toml")?
        .version()
        .parse()
        .map_err(anyhow::Error::msg)?)
}

fn read_wasm_file(work_dir: &Path, pkg_name: &str) -> Result<Vec<u8>> {
    // WASM-File Pfad ermitteln
    let wasm_path = work_dir
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("{}.wasm", pkg_name));

    // Prüfen ob File existiert
    if !wasm_path.exists() {
        // TODO: In this case we could try to invoke cargo under the hood and compile the binary
        bail!("WASM file not found: {}", wasm_path.display());
    }

    // Als Vec<u8> einlesen
    let wasm_bytes = fs::read(&wasm_path)
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
//     fs::write(path.join("package.json"), json)?;
//     info(format!("Bundle saved to {}", path.display()))?;
//     Ok(())
// }

fn compile_project(work_dir: &std::path::Path) -> Result<()> {
    let sp = spinner();
    sp.start("Compiling to WebAssembly...");

    let child = std::process::Command::new("cargo")
        .args(["build", "--release", "--target=wasm32-unknown-unknown"])
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
    } else {
        sp.stop("Build failed");
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("WASM build failed:\n{}", stderr);
    }

    Ok(())
}
