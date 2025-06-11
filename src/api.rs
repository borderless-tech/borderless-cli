use std::{
    fs,
    io::{BufRead, Write},
    path::PathBuf,
};

use anyhow::{bail, Context, Result};
use borderless::{common::Introduction, BorderlessId};
use cliclack::{
    log::{info, warning},
    select,
};
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::config;

// NOTE: We have to greatly expand this,
// because a link should also consist of information about the certificate,
// peer-id, organization behind the node etc.
//
// But for no we make this easy. A linked node has a name, an API-address and API-Key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub name: String,
    pub api: Url,
    pub api_key: Option<String>,
}

impl Link {
    pub fn to_string(&self) -> String {
        format!("{} - {}", self.name, self.api)
    }
}

// NOTE: This is a very naive and easy implementation,
// which should be very sufficient for a relatively long time.
// (we don't require a fully fledged database here)
#[derive(Debug, Clone)]
pub struct LinkDb {
    db: PathBuf,
    // Buffered links
    links: Vec<Link>,
}

impl LinkDb {
    /// Opens the `LinkDb` and parses all its content
    pub fn open() -> Result<Self> {
        let data_home = config::get_config().data_dir()?;
        let db = data_home.join("LINKS");
        if !db.exists() {
            fs::File::create(&db)?;
        } else if !db.is_file() {
            bail!("link-file '{}' must be a file", db.display());
        }
        // Read file line by line
        let content = fs::read(&db)?;
        let mut links = Vec::new();
        for line in content.lines() {
            let link = serde_json::from_str(&line?).context(format!(
                "corrupted data - consider removing '{}'",
                db.display()
            ))?;
            links.push(link);
        }

        Ok(Self { db, links })
    }

    /// Returns the links
    pub fn get_links(&self) -> Vec<Link> {
        self.links.clone()
    }

    /// Returns true if a link with the given name already exists
    pub fn contains(&self, name: &str) -> bool {
        self.links.iter().find(|l| l.name == name).is_some()
    }

    /// Modifies an existing link by its name
    pub fn modify_link(&mut self, name: &str, new_link: Link) -> Result<()> {
        self.remove_link(name)?;
        self.add_link(new_link);
        Ok(())
    }

    /// Removes a link by its name
    pub fn remove_link(&mut self, name: &str) -> Result<()> {
        let idx = match self.links.iter().enumerate().find(|(_, p)| p.name == name) {
            Some((idx, _)) => idx,
            None => {
                warning(format!("Found no link with name: {name}"))?;
                return Ok(());
            }
        };
        self.links.remove(idx);
        Ok(())
    }

    /// Adds a new link
    pub fn add_link(&mut self, new_link: Link) {
        self.links.push(new_link);
    }

    /// Commits the links to disk
    pub fn commit(self) -> Result<()> {
        let mut file = fs::File::create(self.db)?;
        for link in self.links {
            let encoded = serde_json::to_string(&link)?;
            file.write(encoded.as_bytes())?;
            file.write("\n".as_bytes())?;
        }
        file.flush()?;
        Ok(())
    }
}

pub struct Node {
    link: Link,
}

impl Node {
    pub fn new(link: Link) -> Self {
        Node { link }
    }

    pub fn select() -> Result<Self> {
        let db = LinkDb::open()?;
        let selectable = db.get_links();
        if selectable.is_empty() {
            bail!("There are no nodes are linked to the cli-tool. Use 'borderless link' to create a new link");
        } else if selectable.len() == 1 {
            let link = selectable.into_iter().next().unwrap();
            info(format!("Use node {}", link.to_string()))?;
            return Ok(Node { link });
        }
        let mut prompt = select("Select node:");
        for item in selectable {
            prompt = prompt.item(item.clone(), item.name, item.api.to_string());
        }
        let selection = prompt.filter_mode().interact()?;
        Ok(Node { link: selection })
    }

    /// Writes an introduction
    pub fn write_introduction(&self, introduction: Introduction) -> Result<bool> {
        let endpoint = "/v0/write/introduction";
        let url = self.link.api.join(&endpoint)?;

        let body = introduction.to_bytes()?;

        let client = reqwest::blocking::Client::new();
        let res = client
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()?;

        if !res.status().is_success() {
            return Ok(false);
        }

        let body = res.bytes()?;
        let value: Value = serde_json::from_slice(&body)?;

        let pretty = serde_json::to_string_pretty(&value)?;
        info(pretty)?;

        Ok(true)
    }

    /// Returns the node-info
    pub fn node_info(&self) -> Result<Value> {
        let endpoint = "/v0/node/info";
        let url = self.link.api.join(&endpoint)?;

        let result = reqwest::blocking::get(url)?;
        let body = result.bytes()?;

        let info: Value = serde_json::from_slice(&body)?;
        Ok(info)
    }

    /// Returns the list of network peers for a node
    pub fn network_peers(&self) -> Result<Vec<(String, BorderlessId)>> {
        let endpoint = "/v0/node/cert?node_type=contract";
        let url = self.link.api.join(&endpoint)?;

        let result = reqwest::blocking::get(url)?;
        let body = result.bytes()?;

        // We don't use the real model here, we just now it's a list of something
        let certs: Vec<Value> = serde_json::from_slice(&body)?;

        let mut out = Vec::new();
        for cert in certs {
            let pid: BorderlessId = cert
                .get("participant_id")
                .and_then(|s| s.as_str())
                .unwrap_or_default()
                .parse()?;

            let name: String = cert
                .get("subject")
                .and_then(|s| s.as_str())
                .unwrap_or_default()
                .to_string();
            out.push((name, pid));
        }

        Ok(out)
    }
}
