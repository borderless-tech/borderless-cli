use anyhow::{bail, Context, Result};
use borderless_hash::Hash256;
use borderless_pkg::*;
use cliclack::{
    intro,
    log::{info, success},
    spinner,
};
use convert_case::{Case, Casing};
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr,
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

    info(format!(
        "Working directory set to: {}",
        absolute_path.display()
    ))?;

    // Compile the project (this gives us the target path)
    let target_path = compile_project(&absolute_path)?;

    // read wasm as bytes
    let wasm_bytes = read_wasm_file(&target_path, &pkg_info.name)?;

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
    }
    .into_dto();
    let out = serde_json::to_vec(&pkg)?;

    let pkg_file = path.join("package.json");
    fs::write(&pkg_file, &out)?;

    success(format!(
        "Created package definition for '{}', output = {}",
        pkg_info.name,
        pkg_file.display()
    ))?;
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

/// Reads the wasm binary from the target path
fn read_wasm_file(target_dir: &Path, pkg_name: &str) -> Result<Vec<u8>> {
    let wasm_pkg_name = format!("{}.wasm", pkg_name.to_case(Case::Snake));

    // The target directory was obtained from cargo metadata.
    //
    // If `compile_project` was executed without errors before this function,
    // we should always find a binary at this path:
    let wasm_path = target_dir
        .join("wasm32-unknown-unknown/release")
        .join(wasm_pkg_name);

    // Nonetheless: Check for existence of the binary
    if !wasm_path.exists() {
        bail!(
            "Failed to find wasm binary: '{}' does not exist",
            wasm_path.display()
        );
    }

    // Read bytes from disk
    let wasm_bytes = fs::read(&wasm_path)
        .with_context(|| format!("Failed to read WASM file: {}", wasm_path.display()))?;

    let wasm_file = wasm_path
        .file_name()
        .unwrap_or_else(|| wasm_path.as_os_str())
        .to_string_lossy();

    info(format!(
        "Read binary '{}', size = {}",
        wasm_file,
        human_readable_size(wasm_bytes.len())
    ))?;

    Ok(wasm_bytes)
}

// Helper function to pretty-print the byte size
fn human_readable_size(size: usize) -> String {
    let units = ["bytes", "KB", "MB", "GB", "TB"];
    let mut size = size as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < units.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, units[unit_index])
}

/// Compiles the project into a wasm binary and returns the target path
fn compile_project(work_dir: &Path) -> Result<PathBuf> {
    let sp = spinner();

    info("Compiling package to WebAssembly...")?;
    sp.start("cargo build --release --target=wasm32-unknown-unknown");

    // Spawn `cargo build ...` with stdout/stderr piped.
    //
    // NOTE: Cargo pipes its output to stderr and not to stdout
    let mut child = Command::new("cargo")
        .args(["build", "--release", "--target=wasm32-unknown-unknown"])
        .current_dir(work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to start `cargo build`")?;

    let stdout = child
        .stdout
        .take()
        .context("Failed to capture stdout of cargo")?;
    let stderr = child
        .stderr
        .take()
        .context("Failed to capture stderr of cargo")?;

    // Wrap stdout in a line‐buffered reader:
    let mut _stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    // Read lines from stderr as they arrive and update spinner
    while let Some(line_res) = stderr_reader.next() {
        let line = line_res.unwrap_or_else(|e| format!("failed to read cargo output: {e}"));
        sp.set_message(&line);
    }

    // Wait for the child to exit, so we can check exit status.
    let status = child.wait().context("Failed to wait for cargo to finish")?;

    if !status.success() {
        sp.stop("Build failed");
        // If you also want stderr details, you can decode `output.stderr`:
        // let stderr_text = String::from_utf8_lossy(&output.stderr);
        bail!("WASM build failed",);
    }

    // Now obtain the cargo metadata to retrieve the compilation path
    sp.set_message("Reading cargo metadata...");
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version=1"])
        .current_dir(work_dir)
        .output()
        .context("Failed to run `cargo metadata`")?;
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .context("failed to read output of `cargo metadata`")?;

    let target_path = metadata
        .get("target_directory")
        .and_then(|v| v.as_str())
        .and_then(|s| PathBuf::from_str(s).ok())
        .unwrap_or_else(|| work_dir.join("target"))
        .canonicalize()?;

    sp.stop("WASM build completed successfully.");

    Ok(target_path)
}
