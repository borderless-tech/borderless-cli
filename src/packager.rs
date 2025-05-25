use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use borderless_format::{self, Bundle, Contract, Ident, Metadata, Source};
use borderless_hash::Hash256;
use cliclack::log::info;
use ed25519_dalek::{Signer, SigningKey};
use serde_json;
use std::path::{Path, PathBuf};

use crate::template::ContractManifest;

pub(crate) fn pack_wasm_contract(
    manifest: &ContractManifest,
    wasm: &[u8],
    private_key: Option<PathBuf>,
) -> Result<Bundle> {
    info("Pack Smart Contract")?;

    // hash the wasm file
    let wasm_hash = Hash256::digest(&wasm);
    info(format!("Contract Hash: {}", wasm_hash))?;

    // build up the smart contract bundle
    let encoded_contract = general_purpose::STANDARD.encode(&wasm);

    let src = Source {
        hash: wasm_hash,
        wasm: encoded_contract,
        version: manifest.sdk.version.clone(),
        compiler: "".to_string(),
    };

    let meta = Metadata {
        did: "".to_string(),
        name: manifest.contract.name.clone(),
        version: manifest.contract.version.clone(),
        authors: vec![manifest.contract.author.clone()],
        description: manifest.contract.desc.clone(),
    };

    let contract = Contract { meta, src };

    let ident: Option<Ident> = if private_key.is_some() {
        let keypair = load_pem_private_key(private_key.unwrap().as_path())?;
        let json = serde_json::to_string(&contract)?;
        let signature = keypair.sign(json.as_bytes());

        Some(Ident {
            public_key: hex::encode(keypair.verifying_key().to_bytes()),
            signature: hex::encode(signature.to_bytes()),
        })
    } else {
        None
    };

    let bundle = Bundle { contract, ident };
    Ok(bundle)
}

pub(crate) fn load_pem_private_key(key_path: &Path) -> Result<SigningKey> {
    let pem_content = std::fs::read_to_string(key_path)
        .with_context(|| format!("Failed to read PEM file: {}", key_path.display()))?;

    // PEM parsen
    let pem = pem::parse(&pem_content).context("Failed to parse PEM file")?;

    info(format!("PEM tag: {}", pem.tag()))?;

    // Verschiedene PEM-Formate unterstützen
    let keypair = match pem.tag() {
        "PRIVATE KEY" => parse_pkcs8_private_key(&pem.contents())?,
        "ED25519 PRIVATE KEY" => parse_raw_ed25519_private_key(&pem.contents())?,
        _ => bail!("Unsupported PEM tag: {}", pem.tag()),
    };

    info("Ed25519 private key loaded from PEM")?;
    info(format!(
        "Public key: {}",
        hex::encode(keypair.verifying_key().to_bytes())
    ))?;

    Ok(keypair)
}

fn parse_pkcs8_private_key(der_bytes: &[u8]) -> Result<SigningKey> {
    if der_bytes.len() < 32 {
        bail!("Invalid PKCS#8 private key: too short");
    }

    let key_start = der_bytes.len().saturating_sub(32);
    let secret_bytes = &der_bytes[key_start..];

    if secret_bytes.len() != 32 {
        bail!("Invalid Ed25519 private key length in PKCS#8");
    }

    let mut secret = [0u8; 32];
    secret.copy_from_slice(secret_bytes);

    let secret_key = SigningKey::from_bytes(&secret);
    Ok(SigningKey::from(secret_key))
}

fn parse_raw_ed25519_private_key(der_bytes: &[u8]) -> Result<SigningKey> {
    if der_bytes.len() != 32 {
        bail!(
            "Invalid Ed25519 private key: expected 32 bytes, got {}",
            der_bytes.len()
        );
    }

    let mut secret = [0u8; 32];
    secret.copy_from_slice(der_bytes);

    let secret_key = SigningKey::from_bytes(&secret);
    Ok(SigningKey::from(secret_key))
}
