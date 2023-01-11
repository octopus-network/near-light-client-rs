#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod near_types;

use alloc::vec::Vec;
use borsh::{BorshDeserialize, BorshSerialize};
use near_types::{
    get_raw_prefix_for_contract_data,
    hash::{sha256, CryptoHash},
    merkle::{compute_root_from_path, merklize, MerklePath},
    signature::{PublicKey, Signature},
    transaction::ExecutionOutcomeWithId,
    trie::{verify_state_proof, RawTrieNodeWithSize},
    AccountId, BlockId, LightClientBlockLiteView, LightClientBlockView, ValidatorStakeView,
};

/// The head data struct of NEAR light client.
///
/// It's a wrapper of NEAR defined type `LightClientBlockView`,
/// adding an array of `prev_state_root` of chunks at the same height.
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct LightClientBlockViewExt {
    pub light_client_block_view: LightClientBlockView,
    pub prev_state_root_of_chunks: Vec<CryptoHash>,
}

/// Error type for function `validate_and_update_head`.
#[derive(Debug, Clone)]
pub enum ClientStateVerificationError {
    UninitializedClient,
    InvalidBlockHeight,
    InvalidEpochId,
    MissingNextBlockProducersInHead,
    MissingCachedEpochBlockProducers {
        epoch_id: CryptoHash,
    },
    InvalidValidatorSignature {
        signature: Signature,
        pubkey: PublicKey,
    },
    BlockIsNotFinal,
    InvalidNextBlockProducersHash,
    InvalidPrevStateRootOfChunks,
}

/// Error type for function `validate_contract_state`.
#[derive(Debug, Clone)]
pub enum ContractStateValidationError {
    MissingHeadInClient { block_id: BlockId },
    InvalidRootHashOfProofData,
    InvalidLeafNodeHash { proof_index: u16 },
    InvalidLeafNodeKey { proof_index: u16 },
    InvalidLeafNodeValueHash { proof_index: u16 },
    InvalidExtensionNodeHash { proof_index: u16 },
    InvalidExtensionNodeKey { proof_index: u16 },
    InvalidBranchNodeHash { proof_index: u16 },
    InvalidBranchNodeValueHash { proof_index: u16 },
    MissingBranchNodeValue { proof_index: u16 },
    MissingBranchNodeChildHash { proof_index: u16 },
    InvalidProofData,
}

/// Error type for function `validate_transaction`.
#[derive(Debug, Clone)]
pub enum TransactionValidationError {
    MissingHeadInClient { block_id: BlockId },
    InvalidOutcomeProof,
    InvalidBlockProof,
}

/// This trait defines all necessary interfaces/functions for NEAR light client.
///
/// It is used to decouple the persistence logic and validation logic of NEAR light client.
pub trait NearLightClient {
    /// Returns the latest light client head.
    fn get_latest_head(&self) -> Option<LightClientBlockViewExt>;

    /// Updates light client head.
    ///
    /// The implementation of this function should also
    /// - cache the head within a certain range of heights. See function `get_head_at` too.
    /// - store the block producers of next epoch in the head data.
    ///   See function `epoch_block_producers` too.
    ///
    /// This function will be called in function `validate_and_update_head` automatically
    /// if all checks are passed.
    ///
    /// As the function `validate_and_update_head` needs cached block producers of current epoch
    /// and the next epoch, a new light client has to wait at most one epoch before function
    /// `validate_and_update_head` can be called successfully. In this period, the client can
    /// use this function to update head directly (with trusted head data).
    fn update_head(&mut self, head: LightClientBlockViewExt);

    /// Returns the block producers corresponding to the given `epoch_id`.
    ///
    /// The light client implementation should cache at least two epoch block producers
    /// (current epoch and the next) for the validation of new head can succeed.
    fn get_epoch_block_producers(&self, epoch_id: &CryptoHash) -> Option<Vec<ValidatorStakeView>>;

    /// Returns the head with the given `BlockId`.
    ///
    /// As the validation of contract state proof or transaction/receipt proof needs
    /// the state root data or outcome root data corresponding to a height or block hash,
    /// the light client implementation should cache a numbers of heads for this
    /// and provide a view function to return the cached block ids for querying.
    fn get_head(&self, block_id: &BlockId) -> Option<LightClientBlockViewExt>;

    /// Validate the given head with `latest_head`.
    ///
    /// Implemented based on the spec at `https://nomicon.io/ChainSpec/LightClient` basically. And
    /// - Added checking of sigatures' count in `approvals_after_next` in head.
    /// - Added checking of `prev_state_root` of chunks for contract state proof validation.
    fn validate_and_update_head(
        &mut self,
        new_head: LightClientBlockViewExt,
    ) -> Result<(), ClientStateVerificationError> {
        let latest_head = self.get_latest_head();
        if latest_head.is_none() {
            return Err(ClientStateVerificationError::UninitializedClient);
        }
        let latest_head = latest_head.unwrap();
        let approval_message = new_head.light_client_block_view.approval_message();

        // Check the height of the block is higher than the height of the current head.
        if new_head.light_client_block_view.inner_lite.height
            <= latest_head.light_client_block_view.inner_lite.height
        {
            return Err(ClientStateVerificationError::InvalidBlockHeight);
        }

        // Check the epoch of the block is equal to the epoch_id or next_epoch_id
        // known for the current head.
        if new_head.light_client_block_view.inner_lite.epoch_id
            != latest_head.light_client_block_view.inner_lite.epoch_id
            && new_head.light_client_block_view.inner_lite.epoch_id
                != latest_head.light_client_block_view.inner_lite.next_epoch_id
        {
            return Err(ClientStateVerificationError::InvalidEpochId);
        }

        // If the epoch of the block is equal to the next_epoch_id of the head,
        // then next_bps is not None.
        if new_head.light_client_block_view.inner_lite.epoch_id
            == latest_head.light_client_block_view.inner_lite.next_epoch_id
            && new_head.light_client_block_view.next_bps.is_none()
        {
            return Err(ClientStateVerificationError::MissingNextBlockProducersInHead);
        }

        // 1. The approvals_after_next contains valid signatures on approval_message
        // from the block producers of the corresponding epoch.
        // 2. The signatures present in approvals_after_next correspond to
        // more than 2/3 of the total stake.
        let mut total_stake = 0;
        let mut approved_stake = 0;

        let bps =
            self.get_epoch_block_producers(&new_head.light_client_block_view.inner_lite.epoch_id);
        if bps.is_none() {
            return Err(
                ClientStateVerificationError::MissingCachedEpochBlockProducers {
                    epoch_id: new_head.light_client_block_view.inner_lite.epoch_id,
                },
            );
        }

        let epoch_block_producers = bps.unwrap();
        for (maybe_signature, block_producer) in new_head
            .light_client_block_view
            .approvals_after_next
            .iter()
            .zip(epoch_block_producers.iter())
        {
            let bp_stake_view = block_producer.clone().into_validator_stake();
            let bp_stake = bp_stake_view.stake;
            total_stake += bp_stake;

            if maybe_signature.is_none() {
                continue;
            }

            approved_stake += bp_stake;

            let validator_public_key = bp_stake_view.public_key.clone();
            if !maybe_signature
                .as_ref()
                .unwrap()
                .verify(&approval_message, &validator_public_key)
            {
                return Err(ClientStateVerificationError::InvalidValidatorSignature {
                    signature: maybe_signature.clone().unwrap(),
                    pubkey: validator_public_key,
                });
            }
        }

        let threshold = total_stake * 2 / 3;
        if approved_stake <= threshold {
            return Err(ClientStateVerificationError::BlockIsNotFinal);
        }

        // If next_bps is not none, sha256(borsh(next_bps)) corresponds to
        // the next_bp_hash in inner_lite.
        if new_head.light_client_block_view.next_bps.is_some() {
            let block_view_next_bps_serialized = new_head
                .light_client_block_view
                .next_bps
                .as_deref()
                .unwrap()
                .try_to_vec()
                .unwrap();
            if sha256(&block_view_next_bps_serialized).as_slice()
                != new_head
                    .light_client_block_view
                    .inner_lite
                    .next_bp_hash
                    .as_ref()
            {
                return Err(ClientStateVerificationError::InvalidNextBlockProducersHash);
            }
        }

        // Check the `prev_state_root` is the merkle root of `prev_state_root_of_chunks`.
        if new_head.light_client_block_view.inner_lite.prev_state_root
            != merklize(&new_head.prev_state_root_of_chunks).0
        {
            return Err(ClientStateVerificationError::InvalidPrevStateRootOfChunks);
        }

        self.update_head(new_head);
        Ok(())
    }

    /// Validate the value of a certain storage key of an contract account
    /// corresponding to a certain block id with proof data.
    ///
    /// The `proofs` must be the proof data at `height - 1`.
    fn validate_contract_state(
        &self,
        block_id: &BlockId,
        contract_id: &AccountId,
        key_prefix: &[u8],
        value: &[u8],
        proofs: &Vec<Vec<u8>>,
    ) -> Result<(), ContractStateValidationError> {
        if let Some(head) = self.get_head(block_id) {
            let root_hash = CryptoHash(sha256(proofs[0].as_ref()));
            if !head.prev_state_root_of_chunks.contains(&root_hash) {
                return Err(ContractStateValidationError::InvalidRootHashOfProofData);
            }
            let nodes: Vec<RawTrieNodeWithSize> = proofs
                .iter()
                .map(|bytes| RawTrieNodeWithSize::decode(bytes).unwrap())
                .collect();
            return verify_state_proof(
                &get_raw_prefix_for_contract_data(contract_id, key_prefix),
                &nodes,
                value,
                &root_hash,
            );
        } else {
            return Err(ContractStateValidationError::MissingHeadInClient {
                block_id: block_id.clone(),
            });
        }
    }

    /// Validate the given transaction or receipt outcome with proof data.
    fn validate_transaction_or_receipt(
        &self,
        head_hash: &CryptoHash,
        outcome_with_id: &ExecutionOutcomeWithId,
        outcome_proof: &MerklePath,
        outcome_root_proof: &MerklePath,
        block_lite_view: &LightClientBlockLiteView,
        block_proof: &MerklePath,
    ) -> Result<(), TransactionValidationError> {
        let block_id = BlockId::Hash(*head_hash);
        if let Some(head) = self.get_head(&block_id) {
            let chunk_outcome_root = compute_root_from_path(
                outcome_proof,
                CryptoHash::hash_borsh(&outcome_with_id.to_hashes()),
            );
            let outcome_root = compute_root_from_path(
                outcome_root_proof,
                CryptoHash::hash_borsh(&chunk_outcome_root),
            );
            if outcome_root != block_lite_view.inner_lite.outcome_root {
                return Err(TransactionValidationError::InvalidOutcomeProof);
            }
            let block_merkle_root =
                compute_root_from_path(block_proof, block_lite_view.current_block_hash());
            if block_merkle_root == head.light_client_block_view.inner_lite.block_merkle_root {
                return Ok(());
            } else {
                return Err(TransactionValidationError::InvalidBlockProof);
            }
        } else {
            return Err(TransactionValidationError::MissingHeadInClient { block_id });
        }
    }
}
