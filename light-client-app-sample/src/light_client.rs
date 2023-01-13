//! LightClient implementation
//!

pub mod near_rpc_client_wrapper;
pub mod utils;

use std::collections::{HashMap, VecDeque};

use borsh::{BorshDeserialize, BorshSerialize};
use near_light_client::{
    near_types::{hash::CryptoHash, BlockHeight, ValidatorStakeView},
    types::{ConsensusState, Height},
    BasicNearLightClient,
};

const HEAD_DATA_SUB_FOLDER: &str = "head";

#[derive(BorshDeserialize, BorshSerialize)]
struct BlockProducers(Vec<ValidatorStakeView>);

///
pub struct LightClient {
    base_folder: String,
    cached_heights: VecDeque<BlockHeight>,
}

impl BasicNearLightClient for LightClient {
    fn latest_height(&self) -> Height {
        self.cached_heights.back().map_or(0, |h| *h)
    }

    fn get_consensus_state(&self, height: &Height) -> Option<ConsensusState> {
        let file_name = format!("{}/{}/{}", self.base_folder, HEAD_DATA_SUB_FOLDER, height);
        if let Ok(bytes) = std::fs::read(file_name) {
            return Some(
                ConsensusState::try_from_slice(&bytes)
                    .expect(format!("Invalid head data file for height {}.", height).as_str()),
            );
        }
        None
    }
}

impl LightClient {
    /// Create light client from a trusted head
    pub fn new(base_folder: String) -> Self {
        let (queue, _map) = get_cached_heights(&base_folder);
        LightClient {
            base_folder: base_folder.clone(),
            cached_heights: queue,
        }
    }
    ///
    pub fn oldest_height(&self) -> Option<u64> {
        self.cached_heights.front().map(|h| *h)
    }
    ///
    pub fn cached_heights(&self) -> Vec<u64> {
        self.cached_heights.iter().map(|h| *h).collect()
    }
    ///
    pub fn set_consensus_state(&mut self, height: &Height, consensus_state: ConsensusState) {
        let file_name = format!("{}/{}/{}", self.base_folder, HEAD_DATA_SUB_FOLDER, height);
        std::fs::write(file_name, consensus_state.try_to_vec().unwrap())
            .expect("Failed to save light client state to file.");
    }
    ///
    pub fn remove_oldest_head(&mut self) {
        if let Some(height) = self.cached_heights.pop_front() {
            let file_name = format!("{}/{}/{}", self.base_folder, HEAD_DATA_SUB_FOLDER, height);
            std::fs::remove_file(file_name)
                .expect(format!("Failed to remove head data file for height {}.", height).as_str());
        }
    }
    ///
    pub fn save_failed_head(&self, head: ConsensusState) {
        let file_name = format!(
            "{}/failed_head/{}",
            self.base_folder, head.header.light_client_block_view.inner_lite.height
        );
        std::fs::write(file_name, head.try_to_vec().unwrap())
            .expect("Failed to save failed light client head to file.");
    }
}

//
fn get_cached_heights(
    base_folder: &String,
) -> (VecDeque<BlockHeight>, HashMap<CryptoHash, BlockHeight>) {
    let head_data_path = format!("{}/{}", base_folder, HEAD_DATA_SUB_FOLDER);
    let mut heights = Vec::new();
    let mut result_map = HashMap::new();
    for entry in std::fs::read_dir(head_data_path).expect("Failed to access head data folder.") {
        let dir_entry = entry.expect("Invalid file entry.");
        let path = dir_entry.path();
        if path.is_file() {
            if let Ok(bytes) = std::fs::read(path.as_os_str()) {
                let head = ConsensusState::try_from_slice(&bytes)
                    .expect(format!("Invalid head data file {}.", path.display()).as_str());
                heights.push(head.header.light_client_block_view.inner_lite.height);
                let current_block_hash = head.header.light_client_block_view.current_block_hash();
                result_map.insert(
                    current_block_hash,
                    head.header.light_client_block_view.inner_lite.height,
                );
            }
        }
    }
    heights.sort();
    let mut result = VecDeque::new();
    heights.iter().for_each(|h| result.push_back(*h));
    (result, result_map)
}
