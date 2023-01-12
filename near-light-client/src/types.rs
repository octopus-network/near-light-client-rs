use alloc::{string::String, vec::Vec};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::near_types::{hash::CryptoHash, LightClientBlockView, ValidatorStakeView};

pub type ClientIdentifier = String;
pub type Height = u64;

/// The header data struct of NEAR light client.
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct Header {
    pub light_client_block_view: LightClientBlockView,
    pub prev_state_root_of_chunks: Vec<CryptoHash>,
}

/// The consensus state of NEAR light client.
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct ConsensusState {
    /// Block producers of current epoch
    pub current_bps: Vec<ValidatorStakeView>,
    /// Header data
    pub header: Header,
}

/// The state of NEAR light client.
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct ClientState {
    pub trusting_period: u64,
    pub latest_height: Height,
    pub latest_timestamp: u64,
    pub upgrade_commitment_prefix: Vec<u8>,
    /// Storage key for upgrading data
    pub upgrade_key: Vec<u8>,
}
