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
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
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
    }
    .into_dto();
    let out = serde_json::to_vec(&pkg)?;

    let pkg_file = path.join("package.json");
    fs::write(pkg_file, &out)?;
    info(format!("Saved package.json"))?;

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

fn compile_project(work_dir: &std::path::Path) -> Result<()> {
    // Clean build
    // let _status = Command::new("cargo")
    //     .current_dir(work_dir)
    //     .args(["clean"])
    //     .stdout(Stdio::null())
    //     .stderr(Stdio::null())
    //     .spawn()?
    //     .wait()?;

    let sp = spinner();

    info("Compiling package to WebAssembly...")?;
    sp.start("cargo build --release --target=wasm32-unknown-unknown");

    // 2) Spawn `cargo build ...` with stdout/stderr piped.
    let mut child = Command::new("cargo")
        .args(["build", "--release", "--target=wasm32-unknown-unknown"])
        .current_dir(work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to start `cargo build`")?;

    // 3) Take ownership of stdout (and stderr if you like). Here we'll read from stdout.
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
    // (Optional) If you want to show stderr lines as well, you can spawn a thread or merge them.

    // 4) Read lines as they arrive. Each time Cargo prints a new line, update the spinner’s message.
    while let Some(line_res) = stderr_reader.next() {
        let line = line_res.context("Failed to read line from cargo stdout")?;
        // Set the spinner message to the "current" Cargo line:
        sp.set_message(&line);
    }
    // At this point, stdout has closed (Cargo is done printing to stdout).

    // 5) Wait for the child to exit, so we can check exit status.
    let status = child.wait().context("Failed to wait for cargo to finish")?;

    // 6) Stop the spinner and bail or succeed based on exit status.
    if status.success() {
        sp.stop("WASM build completed successfully.");
        Ok(())
    } else {
        sp.stop("Build failed");
        // If you also want stderr details, you can decode `output.stderr`:
        // let stderr_text = String::from_utf8_lossy(&output.stderr);
        bail!("WASM build failed",);
    }
}
