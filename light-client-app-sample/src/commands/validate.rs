//! `start` subcommand - example of how to write a subcommand

use std::convert::TryFrom;
use std::ops::Deref;

use crate::light_client::{near_rpc_client_wrapper::NearRpcClientWrapper, LightClient};
/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::prelude::*;
use abscissa_core::{Command, Runnable};
use near_light_client::near_types::trie::RawTrieNodeWithSize;
use near_light_client::NearLightClient;
use near_primitives::types::{AccountId, BlockId};

/// `start` subcommand
///
/// The `Parser` proc macro generates an option parser based on the struct
/// definition, and is defined in the `clap` crate. See their documentation
/// for a more comprehensive example:
///
/// <https://docs.rs/clap/>
#[derive(clap::Parser, Command, Debug)]
pub struct ValidateCmd {
    pub block_height: u64,
    pub near_account: String,
    /// base64 formatted storage key
    pub storage_key: String,
    /// base64 formatted value
    pub value: String,
}

impl Runnable for ValidateCmd {
    /// Start the application.
    fn run(&self) {
        abscissa_tokio::run(
            &APP,
            validate_storage_state(
                self.block_height,
                &self.near_account,
                &self.storage_key,
                &self.value,
            ),
        )
        .expect("Failed to print status of NEAR light client.");
    }
}

async fn validate_storage_state(
    block_height: u64,
    near_account: &String,
    storage_key: &String,
    value: &String,
) {
    let light_client = LightClient::new(APP.config().state_data.data_folder.clone());
    let head = light_client.get_head_at(block_height);
    if head.is_none() {
        status_err!("Missing head data at height {}.", block_height);
        return;
    }
    let rpc_client = NearRpcClientWrapper::new(APP.config().near_rpc.rpc_endpoint.as_str());
    let key_bytes = base64::decode(storage_key).unwrap();
    let result = rpc_client
        .view_state_with_proof(
            AccountId::try_from(near_account.clone()).unwrap(),
            Some(key_bytes.as_ref()),
            Some(BlockId::Height(block_height - 1)),
        )
        .await
        .expect("Failed to view state of the given NEAR account.");
    assert!(result.values.len() > 0, "Invalid storage key.");
    assert!(
        result.values.len() == 1,
        "The storage key is mapped to multiple values."
    );
    let value_bytes = base64::decode(value).unwrap();
    assert_eq!(
        result.values[0].value.deref(),
        value_bytes.deref(),
        "The value on chain is different from the given value."
    );
    let proofs: Vec<Vec<u8>> = result.proof.iter().map(|proof| proof.to_vec()).collect();
    status_info!("Validating", "Proof data array length: {}", proofs.len());
    let nodes: Vec<RawTrieNodeWithSize> = proofs
        .iter()
        .map(|bytes| RawTrieNodeWithSize::decode(bytes).unwrap())
        .collect();
    status_info!("Validating", "Proof data decoded: {:?}", nodes);
    match light_client.validate_contract_state(
        block_height,
        near_account,
        key_bytes.as_ref(),
        value_bytes.as_ref(),
        &proofs,
    ) {
        Ok(()) => status_ok!("Finished", "Validation succeeded."),
        Err(err) => status_err!(format!("{:?}", err)),
    }
}
