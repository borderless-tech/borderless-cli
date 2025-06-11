use anyhow::{Context, Result};
use borderless_pkg::{Capabilities, PkgMeta, PkgType};
use convert_case::{Case, Casing};
use rust_embed::Embed;
use serde::{Deserialize, Serialize};

/// All of our templates
#[derive(Embed)]
#[folder = "templates/"]
struct Templates;

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub agent: Option<PkgInfo>,
    pub contract: Option<PkgInfo>,
    pub capabilities: Option<Capabilities>,
    pub meta: Option<PkgMeta>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PkgInfo {
    pub name: String,
    pub app_name: Option<String>,
    pub app_module: Option<String>,
}

pub fn generate_manifest(
    pkg_name: &str,
    pkg_type: &PkgType,
    authors: Vec<String>,
) -> Result<String> {
    let manifest_template = match pkg_type {
        PkgType::Contract => Templates::get("manifest-contract.toml"),
        PkgType::Agent => Templates::get("manifest-agent.toml"),
    }
    .context("missing manifest template")?
    .data
    .to_vec();

    // Build authors expression for manifest
    let authors: Vec<_> = authors.into_iter().map(|s| format!("\"{s}\"")).collect();
    let authors_expr = format!("[ {} ]", authors.join(", "));
    let name_expr = format!("\"{pkg_name}\"");

    // Build manifest from template
    let manifest = String::from_utf8(manifest_template)?
        .replace("__NAME__", &name_expr)
        .replace("__AUTHORS__", &authors_expr);
    Ok(manifest)
}

pub fn generate_lib_rs(pkg_name: &str, pkg_type: &PkgType) -> Result<String> {
    let lib_template = match pkg_type {
        PkgType::Contract => Templates::get("init-lib-contract.rs"),
        PkgType::Agent => Templates::get("init-lib-agent.rs"),
    }
    .context("missing lib.rs template")?
    .data
    .to_vec();

    let module_name = pkg_name.to_case(Case::Snake);
    let state_name = pkg_name.to_case(Case::Pascal);

    let lib = String::from_utf8(lib_template)?
        .replace("__module_name__", &module_name)
        .replace("__StateName__", &state_name);
    Ok(lib)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_manifest_template() -> Result<()> {
        let manifest_str = generate_manifest("some-name", &PkgType::Agent, vec![])?;
        // Try parse that
        let manifest: Manifest = toml::from_str(&manifest_str)?;
        assert!(manifest.agent.is_some());
        assert!(manifest.contract.is_none());
        let agent = manifest.agent.unwrap();
        assert_eq!(agent.name, "some-name");
        Ok(())
    }

    #[test]
    fn contract_manifest_template() -> Result<()> {
        let manifest_str = generate_manifest("some-name", &PkgType::Contract, vec![])?;
        // Try parse that
        let manifest: Manifest = toml::from_str(&manifest_str)?;
        assert!(manifest.agent.is_none());
        assert!(manifest.contract.is_some());
        let contract = manifest.contract.unwrap();
        assert_eq!(contract.name, "some-name");
        Ok(())
    }
}
