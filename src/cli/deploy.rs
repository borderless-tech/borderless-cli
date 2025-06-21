use std::{fs, path::PathBuf};

use anyhow::{bail, Result};
use borderless::common::IntroductionDto;
use cliclack::{intro, outro};

use crate::api::Node;

pub fn handle_deploy(path: PathBuf) -> Result<()> {
    intro("🚀 Preparing to deploy ...")?;

    let node = Node::select()?;

    // Read introduction
    if !path.exists() {
        bail!("{} does not exist", path.display());
    }
    if !path.is_file() {
        bail!("{} is not a file", path.display());
    }
    let content = fs::read(path)?;
    let introduction: IntroductionDto = serde_json::from_slice(&content)?;

    if node.write_introduction(introduction)? {
        outro("Wrote introduction")?;
    } else {
        outro("Failed to write introduction")?;
    }

    Ok(())
}
