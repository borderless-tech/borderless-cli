use anyhow::{bail, Context, Result};
use borderless_hash::Hash256;
use borderless_pkg::*;
use cliclack::{
    confirm, intro,
    log::{error, info, success},
    spinner,
};
use convert_case::{Case, Casing};
use git2::{DescribeFormatOptions, DescribeOptions, Repository, StatusOptions};
use git_info::GitInfo;
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

    // try to get git-info
    let git_info = match get_git_info(&absolute_path) {
        Ok(info) => {
            if confirm(format!(
                "Add git-info '{}' to package.json?",
                info.to_string()
            ))
            .interact()?
            {
                Some(info)
            } else {
                None
            }
        }
        Err(e) => {
            error(e)?;
            None
        }
    };

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
                git_info,
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

/// Opens the repository at `path` (usually `"."`) and returns a `GitInfo` with:
/// - `tag`: the nearest annotated tag (if any),
/// - `commits_past_tag`: the number of commits beyond that tag (if any),
/// - `commit_hash_short`: the short (7-char) hex of HEAD,
/// - `dirty`: whether the working tree is dirty.
///
/// Internally, this uses `git2::Repository::describe` + `DescribeOptions` to get a “describe” string,
/// massaged into our format for `GitInfo::from_str`, then does a separate `git2::StatusOptions` check
/// for “dirty.”
pub fn get_git_info(path: &Path) -> Result<GitInfo> {
    // 1. Open the repo (walks up if `path` is inside a subdirectory).
    let repo = Repository::discover(path)?;

    // 2. Determine whether the working tree is dirty:
    let mut status_opts = StatusOptions::new();
    status_opts
        .include_untracked(true)
        .recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut status_opts))?;
    let is_dirty = statuses.iter().any(|entry| {
        let s = entry.status();
        // Any status flag means “dirty”
        s.is_index_new()
            || s.is_index_modified()
            || s.is_index_deleted()
            || s.is_wt_new()
            || s.is_wt_modified()
            || s.is_wt_deleted()
            || s.is_conflicted()
            || s.is_ignored()
            || s.is_wt_renamed()
            || s.is_wt_typechange()
            || s.is_index_renamed()
            || s.is_index_typechange()
    });

    // 3. Use `describe` to get a “tag-<count>-g<hash>” or fallback to the OID.
    let mut desc_opts = DescribeOptions::new();
    desc_opts
        .describe_tags() // use annotated tags
        .show_commit_oid_as_fallback(true) // if no tag, fall back to commit OID
        .max_candidates_tags(10); // no limit on tag‐candidate distance

    let describe = repo.describe(&desc_opts)?;
    let mut fmt_opts = DescribeFormatOptions::new();
    fmt_opts.dirty_suffix(""); // we’ll append “-dirty” ourselves, so disable any suffix here

    // This yields something like:
    // - "v1.2.0-4-g5a85959"
    // - or, if no tag, something like "5a85959" (an abbreviated OID)
    let base_str = describe.format(Some(&fmt_opts))?;

    // 4. If the repo was dirty, append "-dirty"
    let describe_str = if is_dirty {
        format!("{base_str}-dirty")
    } else {
        base_str
    };

    // 5. Parse that final string via `GitInfo::from_str`.
    //    We return a boxed `dyn Error` so we can propagate both `git2::Error` and
    //    any parsing errors from `GitInfo::from_str` (which yields a `String`).
    let info = describe_str
        .parse::<GitInfo>()
        .map_err(anyhow::Error::msg)
        .context("failed to parse `GitInfo`")?;

    Ok(info)
}
