//! This module defines all necessary types for NEAR light client.
//!
//! Most of the codes in this module are ported from `nearcore` v1.30.0
//! and are applied by necessary changes to remove std dependencies.

pub mod hash;
pub mod merkle;
pub mod signature;
pub mod transaction;
pub mod trie;

use self::{
    hash::{combine_hash, sha256, CryptoHash},
    signature::{PublicKey, Signature},
};
use alloc::{string::String, vec::Vec};
use borsh::{BorshDeserialize, BorshSerialize};

/// This column id is used when storing Key-Value data from a contract on an `account_id`.
pub const CONTRACT_DATA: u8 = 9;
pub const ACCOUNT_DATA_SEPARATOR: u8 = b',';

pub type BlockHeight = u64;
pub type AccountId = String;
pub type Balance = u128;
pub type MerkleHash = CryptoHash;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockId {
    Height(BlockHeight),
    Hash(CryptoHash),
}

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
pub struct LightClientBlockLite {
    pub inner_lite: BlockHeaderInnerLite,
    pub inner_rest_hash: CryptoHash,
    pub prev_block_hash: CryptoHash,
}

impl LightClientBlockLite {
    //
    pub fn current_block_hash(&self) -> CryptoHash {
        combine_hash(
            &combine_hash(
                &CryptoHash(sha256(
                    BlockHeaderInnerLite::from(self.inner_lite.clone())
                        .try_to_vec()
                        .unwrap()
                        .as_ref(),
                )),
                &self.inner_rest_hash,
            ),
            &self.prev_block_hash,
        )
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Eq, PartialEq)]
pub struct EpochId(pub CryptoHash);

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Eq, PartialEq)]
pub struct BlockHeaderInnerLite {
    /// Height of this block.
    pub height: BlockHeight,
    /// Epoch start hash of this block's epoch.
    /// Used for retrieving validator information
    pub epoch_id: EpochId,
    pub next_epoch_id: EpochId,
    /// Root hash of the state at the previous block.
    pub prev_state_root: MerkleHash,
    /// Root of the outcomes of transactions and receipts.
    pub outcome_root: MerkleHash,
    /// Timestamp at which the block was built (number of non-leap-nanoseconds since January 1, 1970 0:00:00 UTC).
    pub timestamp: u64,
    /// Hash of the next epoch block producers set
    pub next_bp_hash: CryptoHash,
    /// Merkle root of block hashes up to the current block.
    pub block_merkle_root: CryptoHash,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ValidatorStakeViewV1 {
    pub account_id: AccountId,
    pub public_key: PublicKey,
    pub stake: Balance,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum ValidatorStakeView {
    V1(ValidatorStakeViewV1),
}

impl ValidatorStakeView {
    pub fn into_validator_stake(self) -> ValidatorStakeViewV1 {
        match self {
            Self::V1(inner) => inner,
        }
    }
}

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct LightClientBlock {
    pub prev_block_hash: CryptoHash,
    pub next_block_inner_hash: CryptoHash,
    pub inner_lite: BlockHeaderInnerLite,
    pub inner_rest_hash: CryptoHash,
    pub next_bps: Option<Vec<ValidatorStakeView>>,
    pub approvals_after_next: Vec<Option<Signature>>,
}

impl LightClientBlock {
    //
    pub fn current_block_hash(&self) -> CryptoHash {
        combine_hash(
            &combine_hash(
                &CryptoHash(sha256(self.inner_lite.try_to_vec().unwrap().as_ref())),
                &self.inner_rest_hash,
            ),
            &self.prev_block_hash,
        )
    }
    //
    pub fn next_block_hash(&self) -> CryptoHash {
        combine_hash(&self.next_block_inner_hash, &self.current_block_hash())
    }
    //
    pub fn approval_message(&self) -> Vec<u8> {
        [
            ApprovalInner::Endorsement(self.next_block_hash())
                .try_to_vec()
                .unwrap()
                .as_ref(),
            (self.inner_lite.height + 2).to_le_bytes().as_ref(),
        ]
        .concat()
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum ApprovalInner {
    Endorsement(CryptoHash),
    Skip(BlockHeight),
}

pub fn get_raw_prefix_for_contract_data(account_id: &AccountId, prefix: &[u8]) -> Vec<u8> {
    let mut res = Vec::with_capacity(1 + account_id.as_bytes().len() + 1 + prefix.len());
    res.push(CONTRACT_DATA);
    res.extend(account_id.as_bytes());
    res.push(ACCOUNT_DATA_SEPARATOR);
    res.extend(prefix);
    res
}
