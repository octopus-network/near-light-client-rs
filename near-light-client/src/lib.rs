#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod near_types;
pub mod types;

use alloc::vec::Vec;
use borsh::BorshSerialize;
use near_types::{
    hash::{sha256, CryptoHash},
    merkle::{compute_root_from_path, merklize, MerklePath},
    signature::{PublicKey, Signature},
    transaction::ExecutionOutcomeWithId,
    trie::{verify_state_proof, RawTrieNodeWithSize},
    LightClientBlockLiteView, ValidatorStakeView,
};
use types::{ConsensusState, Header, Height};

/// Error type for header verification.
#[derive(Debug, Clone)]
pub enum HeaderVerificationError {
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

/// Error type for state proof verification.
#[derive(Debug, Clone)]
pub enum StateProofVerificationError {
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

/// Error type for transaction verification.
#[derive(Debug, Clone)]
pub enum TransactionVerificationError {
    InvalidOutcomeProof,
    InvalidBlockProof,
}

/// This trait is a minimal interface for NEAR light client,
/// providing a few functions for header verification.
pub trait BasicNearLightClient {
    /// Returns the latest height of the client.
    fn latest_height(&self) -> Height;

    /// Returns the consensus state at the given `Height`.
    fn get_consensus_state(&self, height: &Height) -> Option<ConsensusState>;

    /// Verify header data with the consensus state of latest height.
    fn verify_header(&self, header: &Header) -> Result<(), HeaderVerificationError> {
        let latest_consensus_state = self
            .get_consensus_state(&self.latest_height())
            .expect("Should not fail if the light client is initialized properly.");
        let latest_header = &latest_consensus_state.header;

        let approval_message = header.light_client_block_view.approval_message();

        // Check the height of the block is higher than the height of the current head.
        if header.height() <= latest_header.height() {
            return Err(HeaderVerificationError::InvalidBlockHeight);
        }

        // Check the epoch of the block is equal to the epoch_id or next_epoch_id
        // known for the current head.
        if header.epoch_id() != latest_header.epoch_id()
            && header.epoch_id() != latest_header.next_epoch_id()
        {
            return Err(HeaderVerificationError::InvalidEpochId);
        }

        // If the epoch of the block is equal to the next_epoch_id of the head,
        // then next_bps is not None.
        if header.epoch_id() == latest_header.next_epoch_id()
            && header.light_client_block_view.next_bps.is_none()
        {
            return Err(HeaderVerificationError::MissingNextBlockProducersInHead);
        }

        // 1. The approvals_after_next contains valid signatures on approval_message
        // from the block producers of the corresponding epoch.
        // 2. The signatures present in approvals_after_next correspond to
        // more than 2/3 of the total stake.
        let mut total_stake = 0;
        let mut approved_stake = 0;

        let bps = latest_consensus_state.get_block_producers_of(&header.epoch_id());
        if bps.is_none() {
            return Err(HeaderVerificationError::MissingCachedEpochBlockProducers {
                epoch_id: header.epoch_id(),
            });
        }

        let epoch_block_producers = bps.unwrap();
        for (maybe_signature, block_producer) in header
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
                return Err(HeaderVerificationError::InvalidValidatorSignature {
                    signature: maybe_signature.clone().unwrap(),
                    pubkey: validator_public_key,
                });
            }
        }

        if approved_stake * 3 <= total_stake * 2 {
            return Err(HeaderVerificationError::BlockIsNotFinal);
        }

        // If next_bps is not none, sha256(borsh(next_bps)) corresponds to
        // the next_bp_hash in inner_lite.
        if header.light_client_block_view.next_bps.is_some() {
            let block_view_next_bps_serialized = header
                .light_client_block_view
                .next_bps
                .as_deref()
                .unwrap()
                .try_to_vec()
                .unwrap();
            if sha256(&block_view_next_bps_serialized).as_slice()
                != header
                    .light_client_block_view
                    .inner_lite
                    .next_bp_hash
                    .as_ref()
            {
                return Err(HeaderVerificationError::InvalidNextBlockProducersHash);
            }
        }

        // Check the `prev_state_root` is the merkle root of `prev_state_root_of_chunks`.
        if header.light_client_block_view.inner_lite.prev_state_root
            != merklize(&header.prev_state_root_of_chunks).0
        {
            return Err(HeaderVerificationError::InvalidPrevStateRootOfChunks);
        }

        Ok(())
    }
}

impl Header {
    ///
    pub fn height(&self) -> Height {
        self.light_client_block_view.inner_lite.height
    }
    ///
    pub fn epoch_id(&self) -> CryptoHash {
        self.light_client_block_view.inner_lite.epoch_id
    }
    ///
    pub fn next_epoch_id(&self) -> CryptoHash {
        self.light_client_block_view.inner_lite.next_epoch_id
    }
}

impl ConsensusState {
    /// Returns the block producers corresponding to current epoch or the next.
    pub fn get_block_producers_of(&self, epoch_id: &CryptoHash) -> Option<Vec<ValidatorStakeView>> {
        if *epoch_id == self.header.epoch_id() {
            return self.current_bps.clone();
        } else if *epoch_id == self.header.next_epoch_id() {
            return self.header.light_client_block_view.next_bps.clone();
        } else {
            return None;
        }
    }

    /// Verify the value of a certain storage key with proof data.
    ///
    /// The `proofs` must be the proof data at `height - 1`.
    pub fn verify_membership(
        &self,
        key: &[u8],
        value: &[u8],
        proofs: &Vec<Vec<u8>>,
    ) -> Result<(), StateProofVerificationError> {
        let root_hash = CryptoHash(sha256(proofs[0].as_ref()));
        if !self.header.prev_state_root_of_chunks.contains(&root_hash) {
            return Err(StateProofVerificationError::InvalidRootHashOfProofData);
        }
        let nodes: Vec<RawTrieNodeWithSize> = proofs
            .iter()
            .map(|bytes| RawTrieNodeWithSize::decode(bytes).unwrap())
            .collect();
        return verify_state_proof(&key, &nodes, value, &root_hash);
    }

    /// Verify that there is NO value on the path with proof data.
    ///
    /// The `proofs` must be the proof data at `height - 1`.
    pub fn verify_non_membership(
        &self,
        key: &[u8],
        proofs: &Vec<Vec<u8>>,
    ) -> Result<(), StateProofVerificationError> {
        todo!()
    }

    /// Verify the given transaction or receipt outcome with proof data.
    pub fn verify_transaction_or_receipt(
        &self,
        outcome_with_id: &ExecutionOutcomeWithId,
        outcome_proof: &MerklePath,
        outcome_root_proof: &MerklePath,
        block_lite_view: &LightClientBlockLiteView,
        block_proof: &MerklePath,
    ) -> Result<(), TransactionVerificationError> {
        let chunk_outcome_root = compute_root_from_path(
            outcome_proof,
            CryptoHash::hash_borsh(&outcome_with_id.to_hashes()),
        );
        let outcome_root = compute_root_from_path(
            outcome_root_proof,
            CryptoHash::hash_borsh(&chunk_outcome_root),
        );
        if outcome_root != block_lite_view.inner_lite.outcome_root {
            return Err(TransactionVerificationError::InvalidOutcomeProof);
        }
        let block_merkle_root =
            compute_root_from_path(block_proof, block_lite_view.current_block_hash());
        if block_merkle_root
            == self
                .header
                .light_client_block_view
                .inner_lite
                .block_merkle_root
        {
            return Ok(());
        } else {
            return Err(TransactionVerificationError::InvalidBlockProof);
        }
    }
}
