//! `verify-transaction` subcommand
//! Verify a certain transaction with the latest light client head.

use std::convert::TryFrom;
use std::str::FromStr;

use crate::light_client::utils::produce_light_client_block_lite_view;
use crate::light_client::{near_rpc_client_wrapper::NearRpcClientWrapper, LightClient};
/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{info_with_time, prelude::*};
use abscissa_core::{Command, Runnable};
use borsh::BorshDeserialize;
use near_light_client::near_types::hash::CryptoHash;
use near_light_client::near_types::merkle::MerklePathItem;
use near_light_client::near_types::transaction::{
    ExecutionOutcome, ExecutionOutcomeWithId, ExecutionStatus,
};
use near_light_client::BasicNearLightClient;

/// `validate-tx` subcommand
///
/// The `Parser` proc macro generates an option parser based on the struct
/// definition, and is defined in the `clap` crate. See their documentation
/// for a more comprehensive example:
///
/// <https://docs.rs/clap/>
#[derive(clap::Parser, Command, Debug)]
pub struct VerifyTransactionCmd {
    /// base58 formatted transaction hash
    pub tx_hash: String,
    /// Account id of transaction sender
    pub sender_id: String,
}

impl Runnable for VerifyTransactionCmd {
    /// Start the application.
    fn run(&self) {
        abscissa_tokio::run(&APP, validate_transaction(&self.tx_hash, &self.sender_id))
            .expect("Failed to print status of NEAR light client.");
    }
}

async fn validate_transaction(tx_hash: &String, sender_id: &String) {
    let light_client = LightClient::new(APP.config().state_data.data_folder.clone());
    let transaction_hash =
        CryptoHash::try_from(bs58::decode(tx_hash.clone()).into_vec().unwrap().as_ref()).unwrap();
    let sender_id = near_primitives::account::id::AccountId::from_str(sender_id.as_str()).unwrap();
    let rpc_client = NearRpcClientWrapper::new(APP.config().near_rpc.rpc_endpoint.as_str());
    let head = light_client.get_consensus_state(&light_client.latest_height());
    if head.is_none() {
        status_err!("Uninitialized NEAR light client.");
        return;
    }
    let head_state = head.unwrap();
    let head_hash = head_state.header.light_client_block.current_block_hash();
    let result = rpc_client
        .get_light_client_proof(
            &near_primitives::types::TransactionOrReceiptId::Transaction {
                transaction_hash: near_primitives::hash::CryptoHash(transaction_hash.0),
                sender_id,
            },
            &near_primitives::hash::CryptoHash(head_hash.clone().0),
        )
        .await
        .expect("Failed to get light client proof.");
    info_with_time!("Header of block proof: {:?}", result.block_header_lite);
    info_with_time!("Block proof length: {}", result.block_proof.len());
    info_with_time!("Block proof data: {:?}", result.block_proof);
    match head_state.verify_transaction_or_receipt(
        &ExecutionOutcomeWithId {
            id: transaction_hash,
            outcome: ExecutionOutcome {
                logs: result.outcome_proof.outcome.logs,
                receipt_ids: result
                    .outcome_proof
                    .outcome
                    .receipt_ids
                    .iter()
                    .map(|h| CryptoHash(h.clone().0))
                    .collect(),
                gas_burnt: result.outcome_proof.outcome.gas_burnt,
                tokens_burnt: result.outcome_proof.outcome.tokens_burnt,
                executor_id: result.outcome_proof.outcome.executor_id.to_string(),
                status: ExecutionStatus::try_from_slice(
                    borsh::to_vec(&result.outcome_proof.outcome.status)
                        .unwrap()
                        .as_ref(),
                )
                .unwrap(),
            },
        },
        &result
            .outcome_proof
            .proof
            .iter()
            .map(|proof| {
                MerklePathItem::try_from_slice(borsh::to_vec(&proof).unwrap().as_ref()).unwrap()
            })
            .collect(),
        &result
            .outcome_root_proof
            .iter()
            .map(|proof| {
                MerklePathItem::try_from_slice(borsh::to_vec(&proof).unwrap().as_ref()).unwrap()
            })
            .collect(),
        &produce_light_client_block_lite_view(&result.block_header_lite),
        &result
            .block_proof
            .iter()
            .map(|proof| {
                MerklePathItem::try_from_slice(borsh::to_vec(&proof).unwrap().as_ref()).unwrap()
            })
            .collect(),
    ) {
        Ok(()) => status_ok!("Finished", "Validation succeeded."),
        Err(err) => status_err!(format!("{:?}", err)),
    }
}
