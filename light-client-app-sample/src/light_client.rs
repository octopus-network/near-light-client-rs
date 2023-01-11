//! LightClient implementation
//!

pub mod near_rpc_client_wrapper;
pub mod utils;

use std::collections::{HashMap, VecDeque};

use borsh::{BorshDeserialize, BorshSerialize};
use near_light_client::{
    near_types::{hash::CryptoHash, BlockHeight, BlockId, ValidatorStakeView},
    LightClientBlockViewExt, NearLightClient,
};

const HEAD_DATA_SUB_FOLDER: &str = "head";
const BPS_DATA_SUB_FOLDER: &str = "bps";

#[derive(BorshDeserialize, BorshSerialize)]
struct BlockProducers(Vec<ValidatorStakeView>);

///
pub struct LightClient {
    base_folder: String,
    cached_heights: VecDeque<BlockHeight>,
    block_hash_to_height: HashMap<CryptoHash, BlockHeight>,
}

impl NearLightClient for LightClient {
    //
    fn get_latest_head(&self) -> Option<LightClientBlockViewExt> {
        if let Some(height) = self.latest_height() {
            self.get_head(&BlockId::Height(height))
        } else {
            None
        }
    }
    //
    fn update_head(&mut self, head: LightClientBlockViewExt) {
        if let Some(latest_height) = self.cached_heights.back() {
            assert!(
                head.light_client_block_view.inner_lite.height > *latest_height,
                "Head data is too old."
            );
        }
        //
        if let Some(next_bps) = head.light_client_block_view.next_bps.clone() {
            let file_name = format!(
                "{}/{}/{}",
                self.base_folder,
                BPS_DATA_SUB_FOLDER,
                head.light_client_block_view.inner_lite.next_epoch_id
            );
            std::fs::write(file_name, BlockProducers(next_bps).try_to_vec().unwrap())
                .expect("Failed to save light client state to file.");
        }
        //
        let file_name = format!(
            "{}/{}/{}",
            self.base_folder, HEAD_DATA_SUB_FOLDER, head.light_client_block_view.inner_lite.height
        );
        std::fs::write(file_name, head.try_to_vec().unwrap())
            .expect("Failed to save light client state to file.");
        //
        self.cached_heights
            .push_back(head.light_client_block_view.inner_lite.height);
    }
    //
    fn get_epoch_block_producers(&self, epoch_id: &CryptoHash) -> Option<Vec<ValidatorStakeView>> {
        let mut bps: Option<Vec<ValidatorStakeView>> = None;
        let file_name = format!("{}/{}/{}", self.base_folder, BPS_DATA_SUB_FOLDER, epoch_id);
        if let Ok(bytes) = std::fs::read(file_name) {
            bps = Some(
                BlockProducers::try_from_slice(&bytes)
                    .expect(format!("Invalid bps data for epoch id {}.", epoch_id).as_str())
                    .0,
            );
        }
        bps
    }
    //
    fn get_head(&self, block_id: &BlockId) -> Option<LightClientBlockViewExt> {
        let height = match block_id {
            BlockId::Height(height) => *height,
            BlockId::Hash(hash) => {
                if let Some(height) = self.block_hash_to_height.get(hash) {
                    *height
                } else {
                    return None;
                }
            }
        };
        let mut head: Option<LightClientBlockViewExt> = None;
        let file_name = format!("{}/{}/{}", self.base_folder, HEAD_DATA_SUB_FOLDER, height);
        if let Ok(bytes) = std::fs::read(file_name) {
            head = Some(
                LightClientBlockViewExt::try_from_slice(&bytes)
                    .expect(format!("Invalid head data file for height {}.", height).as_str()),
            );
        }
        head
    }
}

impl LightClient {
    /// Create light client from a trusted head
    pub fn new(base_folder: String) -> Self {
        let (queue, map) = get_cached_heights(&base_folder);
        LightClient {
            base_folder: base_folder.clone(),
            cached_heights: queue,
            block_hash_to_height: map,
        }
    }
    ///
    pub fn latest_height(&self) -> Option<u64> {
        self.cached_heights.back().map(|h| *h)
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
    pub fn remove_oldest_head(&mut self) {
        if let Some(height) = self.cached_heights.pop_front() {
            let file_name = format!("{}/{}/{}", self.base_folder, HEAD_DATA_SUB_FOLDER, height);
            std::fs::remove_file(file_name)
                .expect(format!("Failed to remove head data file for height {}.", height).as_str());
        }
    }
    ///
    pub fn save_failed_head(&self, head: LightClientBlockViewExt) {
        let file_name = format!(
            "{}/failed_head/{}",
            self.base_folder, head.light_client_block_view.inner_lite.height
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
                let head = LightClientBlockViewExt::try_from_slice(&bytes)
                    .expect(format!("Invalid head data file {}.", path.display()).as_str());
                heights.push(head.light_client_block_view.inner_lite.height);
                let current_block_hash = head.light_client_block_view.current_block_hash();
                result_map.insert(
                    current_block_hash,
                    head.light_client_block_view.inner_lite.height,
                );
            }
        }
    }
    heights.sort();
    let mut result = VecDeque::new();
    heights.iter().for_each(|h| result.push_back(*h));
    (result, result_map)
}
