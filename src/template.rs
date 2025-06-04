use anyhow::{Context, Result};
use borderless_pkg::{Capabilities, PkgMeta};
use serde::{Deserialize, Serialize};
use std::path::Path;
use toml;

// TODO: Let's use crago-embed to handle the templates

// Dummy definition of a manifest
#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub app_name: Option<String>,
    pub app_module: Option<String>,
    pub capabilities: Option<Capabilities>,
    pub meta: Option<PkgMeta>,
}

const DEFAULT_CONTRACT_MANIFEST: &str = r#"
[Contract]
name = __NAME__
# app_name = "my-fancy-app"
# app_module = "sub-module"

# [Meta]
# authors = [  ]
# description = "a description of your contract"
# documentation = "url to the package documentation"
# license = "SPDX 2.3 license expression"
# repository = "link to the repository"
"#;

const DEFAULT_AGENT_MANIFEST: &str = r#"
[Agent]
name = __NAME__
# app_name = "my-fancy-app"
# app_module = "sub-module"

[Capabilities]
network = true
websocket = true
url_whitelist = []

# [Meta]
# authors = [  ]
# description = "a description of your contract"
# documentation = "url to the package documentation"
# license = "SPDX 2.3 license expression"
# repository = "link to the repository"
"#;

pub fn build_contract_manifest(contract_name: &str) -> String {
    DEFAULT_CONTRACT_MANIFEST.replacen("__NAME__", contract_name, 1)
}

pub fn read_manifest(work_dir: &Path) -> Result<Manifest> {
    let manifest_path = work_dir.join("Manifest.toml");

    let content =
        std::fs::read_to_string(&manifest_path).context("Failed to read Manifest.toml")?;

    let manifest: Manifest = toml::from_str(&content).context("Failed to parse Manifest.toml")?;

    Ok(manifest)
}

pub fn generate_lib_rs() -> String {
    let lib_content = r#"#[borderless::contract]
pub mod flipper {
    use borderless::{Result, *};
    use collections::lazyvec::LazyVec;
    use serde::{Deserialize, Serialize};
    
    #[derive(Serialize, Deserialize)]
    pub struct History {
        switch: bool,
        counter: u32,
    }
    
    // This is our state
    #[derive(State)]
    pub struct Flipper {
        switch: bool,
        counter: u32,
        history: LazyVec<History>,
    }
    
    use self::actions::Actions;
    
    #[derive(NamedSink)]
    pub enum Other {
        Flipper(Actions),
    }
    
    impl Flipper {
        #[action]
        fn flip_switch(&mut self) {
            self.set_switch(!self.switch);
        }
        
        #[action(web_api = true, roles = "Flipper")]
        fn set_switch(&mut self, switch: bool) {
            self.history.push(History {
                switch: self.switch,
                counter: self.counter,
            });
            self.counter += 1;
            self.switch = switch;
        }
    }
}
"#;

    lib_content.to_string()
}
