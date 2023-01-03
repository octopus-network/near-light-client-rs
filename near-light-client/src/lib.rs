#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use borsh::{BorshDeserialize, BorshSerialize};
use near_types::{
    get_raw_prefix_for_contract_data,
    hash::{combine_hash, sha256, CryptoHash},
    merkle::merklize,
    trie::{verify_state_proof, RawTrieNodeWithSize},
    AccountId, ApprovalInner, BlockHeaderInnerLite, BlockHeight, LightClientBlockView,
    ValidatorStakeView,
};

pub mod near_types;

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
    MissingNextBlockProducers,
    InvalidValidatorSignatureCount,
    InvalidValidatorSignature,
    BlockIsNotFinal,
    InvalidNextBlockProducersHash,
    InvalidPrevStateRootOfChunks,
}

/// Error type for function `validate_contract_state_proof`.
#[derive(Debug, Clone)]
pub enum ContractStateValidationError {
    MissingHeadInClient { height: u64 },
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

/// This trait is used to decouple the persistence logic and validation logic of NEAR light client.
pub trait NearLightClient {
    /// Returns the latest head.
    fn latest_head(&self) -> Option<LightClientBlockViewExt>;

    /// Updates the latest head.
    ///
    /// This function should also store the block producers of next epoch in the head,
    /// which will be used in view function `epoch_block_producers`.
    ///
    /// This function will be called in function `validate_and_update_head`
    /// if all checks are passed.
    ///
    /// This function can also be used to initialize or reset the light client
    /// with trusted `LightClientBlockView`.
    fn update_head(&mut self, head: LightClientBlockViewExt);

    /// Returns the block producers corresponding to the given `epoch_id`.
    fn epoch_block_producers(&self, epoch_id: &CryptoHash) -> Option<Vec<ValidatorStakeView>>;

    /// Returns the head at the given height.
    fn get_head_at(&self, height: BlockHeight) -> Option<LightClientBlockViewExt>;

    /// Validate the given head with `latest_head`.
    /// Implemented based on the spec at `https://nomicon.io/ChainSpec/LightClient`.
    fn validate_and_update_head(
        &mut self,
        new_head: LightClientBlockViewExt,
    ) -> Result<(), ClientStateVerificationError> {
        if self.latest_head().is_none() {
            return Err(ClientStateVerificationError::UninitializedClient);
        }
        let latest_head = self.latest_head().unwrap();
        let (_current_block_hash, _next_block_hash, approval_message) =
            reconstruct_light_client_block_view_fields(&new_head.light_client_block_view);

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
            return Err(ClientStateVerificationError::MissingNextBlockProducers);
        }

        // 1. The approvals_after_next contains valid signatures on approval_message
        // from the block producers of the corresponding epoch.
        // 2. The signatures present in approvals_after_next correspond to
        // more than 2/3 of the total stake.
        let mut total_stake = 0;
        let mut approved_stake = 0;

        let epoch_block_producers = &self
            .epoch_block_producers(&new_head.light_client_block_view.inner_lite.epoch_id)
            .expect("Missing epoch block producers. Light client state is invalid.");

        if new_head.light_client_block_view.approvals_after_next.len()
            != epoch_block_producers.len()
        {
            return Err(ClientStateVerificationError::InvalidValidatorSignatureCount);
        }

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
                return Err(ClientStateVerificationError::InvalidValidatorSignature);
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
        if !(new_head.light_client_block_view.inner_lite.prev_state_root
            != merklize(&new_head.prev_state_root_of_chunks).0)
        {
            return Err(ClientStateVerificationError::InvalidPrevStateRootOfChunks);
        }

        self.update_head(new_head);
        Ok(())
    }

    /// Validate the state proof of an contract account at the given height.
    ///
    /// The `proofs` must be the proof data at `height - 1`, which can be queried by
    /// NEAR rpc function `ViewState`.
    fn validate_contract_state_proof(
        &self,
        height: BlockHeight,
        contract_id: AccountId,
        key_prefix: &[u8],
        value: &[u8],
        proofs: Vec<Vec<u8>>,
    ) -> Result<(), ContractStateValidationError> {
        if let Some(head) = self.get_head_at(height) {
            let root_hash = CryptoHash(sha256(proofs[0].as_ref()));
            if !head.prev_state_root_of_chunks.contains(&root_hash) {
                return Err(ContractStateValidationError::InvalidRootHashOfProofData);
            }
            let nodes: Vec<RawTrieNodeWithSize> = proofs
                .iter()
                .map(|bytes| RawTrieNodeWithSize::decode(bytes).unwrap())
                .collect();
            return verify_state_proof(
                &get_raw_prefix_for_contract_data(&contract_id, key_prefix),
                &nodes,
                value,
                &root_hash,
            );
        } else {
            return Err(ContractStateValidationError::MissingHeadInClient { height });
        }
    }
}

fn reconstruct_light_client_block_view_fields(
    block_view: &LightClientBlockView,
) -> (CryptoHash, CryptoHash, Vec<u8>) {
    let current_block_hash = combine_hash(
        &combine_hash(
            &CryptoHash(sha256(
                BlockHeaderInnerLite::from(block_view.inner_lite.clone())
                    .try_to_vec()
                    .unwrap()
                    .as_ref(),
            )),
            &block_view.inner_rest_hash,
        ),
        &block_view.prev_block_hash,
    );
    let next_block_hash = combine_hash(&block_view.next_block_inner_hash, &current_block_hash);
    let approval_message = [
        ApprovalInner::Endorsement(next_block_hash.clone())
            .try_to_vec()
            .unwrap()
            .as_ref(),
        (block_view.inner_lite.height + 2).to_le_bytes().as_ref(),
    ]
    .concat();
    (current_block_hash, next_block_hash, approval_message)
}
