use std::{fmt::format, path::PathBuf};

use anyhow::{bail, Context, Result};
use cliclack::{
    confirm, input, intro,
    log::{info, warning},
    outro, select,
};

use crate::api::Node;

pub fn handle_deploy(path: PathBuf) -> Result<()> {
    intro("🚀 Preparing to deploy ...")?;

    let node = Node::select()?;

    let peers = node.network_peers()?;
    for (name, id) in peers {
        info(format!("{name} - {id}"))?;
    }

    Ok(())
}
