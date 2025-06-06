use std::{
    fs,
    io::BufReader,
    path::{Path, PathBuf},
};

use anyhow::{bail, Result};
use borderless_pkg::WasmPkg;
use cliclack::{confirm, intro, log::success};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::config::get_config;

pub fn handle_merge(introduction_path: PathBuf, package_path: PathBuf) -> Result<()> {
    // Check that introduction exists and is a file
    if !introduction_path.exists() {
        bail!(
            "failed to read introduction at '{}' - file does not exist",
            introduction_path.display()
        );
    }
    if !introduction_path.is_file() {
        bail!("{} is not a file", introduction_path.display());
    }

    // Check that package exists and is a file
    if !package_path.exists() {
        bail!(
            "failed to read package definition at '{}' - file does not exist",
            introduction_path.display()
        );
    }
    if !package_path.is_file() {
        bail!("{} is not a file", package_path.display());
    }

    intro("⟡ Merging package definition into introduction ...")?;

    let mut introduction: Value = read_buffered(&introduction_path)?;

    if let Value::Object(map) = &mut introduction {
        // info(format!("Parsed introduction '{}'", introduction_path.display()))?;
        let package: WasmPkg = read_buffered(&package_path)?;
        // info(format!("Parsed package '{}'", package_path.display()))?;
        let pkg_value = serde_json::to_value(package)?;
        map.insert("package".to_string(), pkg_value);
    } else {
        bail!("introduction must be a json-object");
    }

    // Check, if creation and overwrite requires confirmation
    if get_config().confirm_creation
        && !confirm(format!(
            "This will overwrite the existing introduction at '{}'",
            introduction_path.display()
        ))
        .interact()?
    {
        bail!("Process aborted by user.");
    }

    fs::write(&introduction_path, introduction.to_string())?;

    success(format!(
        "⚭ Merge successful. Wrote new introduction to '{}'",
        introduction_path.display()
    ))?;

    Ok(())
}

fn read_buffered<S: DeserializeOwned>(path: &Path) -> Result<S> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let value = serde_json::from_reader(reader)?;
    Ok(value)
}
