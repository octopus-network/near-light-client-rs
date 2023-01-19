//! `verify-non-membership` subcommand
//! Verify that a certain storage key of a NEAR account has NO value
//! with proof data and optional block height.

use std::convert::TryFrom;

use crate::light_client::{near_rpc_client_wrapper::NearRpcClientWrapper, LightClient};
/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{info_with_time, prelude::*};
use abscissa_core::{Command, Runnable};
use near_light_client::near_types::get_raw_prefix_for_contract_data;
use near_light_client::near_types::trie::RawTrieNodeWithSize;
use near_light_client::BasicNearLightClient;
use near_primitives::types::AccountId;

/// `verify-non-membership` subcommand
///
/// The `Parser` proc macro generates an option parser based on the struct
/// definition, and is defined in the `clap` crate. See their documentation
/// for a more comprehensive example:
///
/// <https://docs.rs/clap/>
#[derive(clap::Parser, Command, Debug)]
pub struct VerifyNonMembershipCmd {
    pub near_account: String,
    /// base64 formatted storage key
    pub storage_key: String,
    pub block_height: Option<u64>,
}

impl Runnable for VerifyNonMembershipCmd {
    /// Start the application.
    fn run(&self) {
        abscissa_tokio::run(
            &APP,
            verify_non_membership(&self.block_height, &self.near_account, &self.storage_key),
        )
        .expect("Failed to print status of NEAR light client.");
    }
}

async fn verify_non_membership(
    block_height: &Option<u64>,
    near_account: &String,
    storage_key: &String,
) {
    let light_client = LightClient::new(APP.config().state_data.data_folder.clone());
    let height = match block_height {
        Some(height) => *height,
        None => light_client.latest_height(),
    };
    let head = light_client.get_consensus_state(&height);
    if head.is_none() {
        status_err!("Missing head data at height {}.", height);
        return;
    }
    let head_state = head.unwrap();
    let rpc_client = NearRpcClientWrapper::new(APP.config().near_rpc.rpc_endpoint.as_str());
    let key_bytes = base64::decode(storage_key).unwrap();
    let result = rpc_client
        .view_state_with_proof(
            AccountId::try_from(near_account.clone()).unwrap(),
            Some(key_bytes.as_ref()),
            Some(near_primitives::types::BlockId::Height(height - 1)),
        )
        .await
        .expect("Failed to view state of the given NEAR account.");
    let proofs: Vec<Vec<u8>> = result.proof.iter().map(|proof| proof.to_vec()).collect();
    info_with_time!("Proof data array length: {}", proofs.len());
    let nodes: Vec<RawTrieNodeWithSize> = proofs
        .iter()
        .map(|bytes| RawTrieNodeWithSize::decode(bytes).unwrap())
        .collect();
    info_with_time!("Proof data decoded: {:?}", nodes);
    match head_state.verify_non_membership(
        &get_raw_prefix_for_contract_data(&near_account, key_bytes.as_ref()),
        &proofs,
    ) {
        Ok(result) => status_ok!("Finished", "Validation result: {}", result),
        Err(err) => status_err!(format!("{:?}", err)),
    }
}
