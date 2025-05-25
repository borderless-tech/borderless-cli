use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use toml;

#[derive(Serialize, Deserialize)]
pub(crate) struct ContractManifest {
    pub contract: ContractInfo,
    pub sdk: SdkInfo,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ContractInfo {
    pub name: String,
    pub version: String,
    pub author: String,
    pub desc: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SdkInfo {
    pub version: String,
}

pub(crate) fn build_contract_manifest(
    contract_name: &str,
    contract_version: &str,
    sdk_version: &str,
) -> Result<String> {
    let manifest = ContractManifest {
        contract: ContractInfo {
            name: contract_name.to_string(),
            version: contract_version.to_string(),
            author: "".to_string(),
            desc: "".to_string(),
        },
        sdk: SdkInfo {
            version: sdk_version.to_string(),
        },
    };

    let toml_string = toml::to_string_pretty(&manifest)?; // Pretty formatting
    Ok(toml_string)
}

pub(crate) fn read_manifest(work_dir: &Path) -> Result<ContractManifest> {
    let manifest_path = work_dir.join("Manifest.toml");

    let content =
        std::fs::read_to_string(&manifest_path).context("Failed to read Manifest.toml")?;

    let manifest: ContractManifest =
        toml::from_str(&content).context("Failed to parse Manifest.toml")?;

    Ok(manifest)
}

pub(crate) fn generate_lib_rs() -> String {
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
