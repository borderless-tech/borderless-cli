use std::fs;

use anyhow::Result;
use borderless::common::Description;
use cliclack::{confirm, intro, log::info, multiselect, outro};
use serde_json::{json, Value};

use crate::{api::Node, TemplateCmd};

pub fn handle_template(cmd: TemplateCmd) -> Result<()> {
    match cmd {
        TemplateCmd::Introduction => create_introduction()?,
    }
    Ok(())
}

fn create_introduction() -> Result<()> {
    intro("Create new introduction template...")?;

    info("We establish a connection to a node to query for participants")?;
    let node = Node::select()?;

    let node_info = node.node_info()?;
    let info_pretty = serde_json::to_string_pretty(&node_info)?;
    info(format!("Node-Info:\n{info_pretty}"))?;

    let peers = node.network_peers()?;

    let mut participants = multiselect("Select peers for contract");

    for (name, id) in peers {
        participants = participants.item(id, format!("{} - {}", name, id), "");
    }
    let participants = participants.filter_mode().interact()?;

    let desc = Description {
        display_name: "".to_string(),
        summary: "".to_string(),
        legal: None,
    };

    let out = json!({
        "participants": participants,
        "initial_state": empty_obj(),
        "roles": [],
        "sinks": [],
        "desc": desc,
        "package": empty_obj(),
    });

    let out_string = serde_json::to_string_pretty(&out)?;

    if confirm("Save as 'introduction.json' ?").interact()? {
        fs::write("./introduction.json", &out_string)?;
    } else {
        info("Template:")?;
        println!("{out_string}");
    }

    outro("Created introduction template. Use 'borderless merge' to merge it with a package definition.")?;

    Ok(())
}

fn empty_obj() -> Value {
    Value::Object(serde_json::Map::default())
}
