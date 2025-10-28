use sc_service::{ChainType, Properties};
use solochain_template_runtime::{TOKEN_DECIMALS, TOKEN_NAME, TOKEN_SYMBOL, WASM_BINARY};

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec;

pub fn development_chain_spec() -> Result<ChainSpec, String> {
    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
        None,
    )
    .with_name("Development")
    .with_id("dev")
    .with_chain_type(ChainType::Development)
    .with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
    .with_properties({
        let mut properties = Properties::new();
        properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());
        properties.insert("tokenName".into(), TOKEN_NAME.into());
        properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
        properties.insert("ss58Format".into(), 42.into());
        properties
    })
    .build())
}

pub fn local_chain_spec() -> Result<ChainSpec, String> {
    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
        None,
    )
    .with_name("Local Testnet")
    .with_id("local_testnet")
    .with_chain_type(ChainType::Local)
    .with_genesis_config_preset_name(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET)
    .with_properties({
        let mut properties = Properties::new();
        properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());
        properties.insert("tokenName".into(), TOKEN_NAME.into());
        properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
        properties.insert("ss58Format".into(), 42.into());
        properties
    })
    .build())
}
