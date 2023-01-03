pub mod hash;
pub mod merkle;
pub mod signature;
pub mod trie;

use self::{
    hash::CryptoHash,
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

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
pub struct BlockHeaderInnerLiteView {
    pub height: BlockHeight,
    pub epoch_id: CryptoHash,
    pub next_epoch_id: CryptoHash,
    pub prev_state_root: CryptoHash,
    pub outcome_root: CryptoHash,
    /// Legacy json number. Should not be used.
    pub timestamp: u64,
    pub timestamp_nanosec: u64,
    pub next_bp_hash: CryptoHash,
    pub block_merkle_root: CryptoHash,
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
pub struct LightClientBlockView {
    pub prev_block_hash: CryptoHash,
    pub next_block_inner_hash: CryptoHash,
    pub inner_lite: BlockHeaderInnerLiteView,
    pub inner_rest_hash: CryptoHash,
    pub next_bps: Option<Vec<ValidatorStakeView>>,
    pub approvals_after_next: Vec<Option<Signature>>,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum ApprovalInner {
    Endorsement(CryptoHash),
    Skip(BlockHeight),
}

impl From<BlockHeaderInnerLiteView> for BlockHeaderInnerLite {
    fn from(view: BlockHeaderInnerLiteView) -> Self {
        BlockHeaderInnerLite {
            height: view.height,
            epoch_id: EpochId(view.epoch_id),
            next_epoch_id: EpochId(view.next_epoch_id),
            prev_state_root: view.prev_state_root,
            outcome_root: view.outcome_root,
            timestamp: view.timestamp_nanosec,
            next_bp_hash: view.next_bp_hash,
            block_merkle_root: view.block_merkle_root,
        }
    }
}

pub fn get_raw_prefix_for_contract_data(account_id: &AccountId, prefix: &[u8]) -> Vec<u8> {
    let mut res = Vec::with_capacity(1 + account_id.as_bytes().len() + 1 + prefix.len());
    res.push(CONTRACT_DATA);
    res.extend(account_id.as_bytes());
    res.push(ACCOUNT_DATA_SEPARATOR);
    res.extend(prefix);
    res
}
