use anyhow::{bail, Context, Result};
use cliclack::{log::info, log::success, spinner};
use std::path::{Path, PathBuf};

pub fn handle_pack(path: PathBuf) -> Result<()> {
    let absolute_path = std::fs::canonicalize(&path).context("Failed to resolve absolute path")?;
    if !absolute_path.is_dir() {
        bail!("Not a directory: {}", absolute_path.display());
    }

    // TODO: Check if contract or agent

    info("Create package")?;
    info(format!("Read folder: {}", path.display()))?;

    info(format!(
        "Working directory set to: {}",
        absolute_path.display()
    ))?;

    // build
    build_wasm(&absolute_path)?;

    let contract_name = "foo";

    // read wasm as bytes
    let _wasm_bytes = read_wasm_file(&absolute_path, contract_name)?;

    // pack contract
    // let bundle = pack_wasm_contract(&manifest, &wasm_bytes, key_path)?;

    // save_bundle_to_file(&bundle, &env::current_dir()?)?;
    success("Contract package created!")?;
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
        // TODO: In this case we could try to invoke cargo under the hood and compile the binary
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
