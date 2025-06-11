use std::{fs, path::PathBuf};

use anyhow::{bail, Context, Result};
use borderless::common::Introduction;
use cliclack::{intro, outro};

use crate::api::Node;

pub fn handle_deploy(path: PathBuf) -> Result<()> {
    intro("🚀 Preparing to deploy ...")?;

    let node = Node::select()?;

    // Read introduction
    if path.exists() {
        bail!("{} does not exist", path.display());
    }
    if path.is_file() {
        bail!("{} is not a file", path.display());
    }
    let content = fs::read(path)?;
    let introduction =
        Introduction::from_bytes(&content).context("failed to parse given introduction file")?;

    if node.write_introduction(introduction)? {
        outro("Wrote introduction")?;
    } else {
        outro("Failed to write introduction")?;
    }

    Ok(())
}
