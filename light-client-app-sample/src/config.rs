//! LightClientAppSample Config
//!
//! See instructions in `commands.rs` to specify the path to your
//! application's configuration file and/or command-line options
//! for specifying it.

use serde::{Deserialize, Serialize};

/// LightClientAppSample Configuration
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LightClientAppSampleConfig {
    /// Configuration for NEAR rpc.
    pub near_rpc: NearRpcSection,
    /// Configuration for state data of NEAR light client.
    pub state_data: StateDataSection,
}

/// Default configuration settings.
///
/// Note: if your needs are as simple as below, you can
/// use `#[derive(Default)]` on LightClientAppSampleConfig instead.
impl Default for LightClientAppSampleConfig {
    fn default() -> Self {
        Self {
            near_rpc: NearRpcSection::default(),
            state_data: StateDataSection::default(),
        }
    }
}

/// Configuration settings for NEAR RPC.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NearRpcSection {
    /// Endpoint of the RPC service. Should be a valid URL.
    pub rpc_endpoint: String,
}

impl Default for NearRpcSection {
    fn default() -> Self {
        Self {
            rpc_endpoint: "https://rpc.testnet.near.org".to_owned(),
        }
    }
}

/// Configuration settings for state data of NEAR light client.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StateDataSection {
    /// The folder which stores state data files.
    pub data_folder: String,
    /// The max height count of cached head data.
    pub max_cached_heights: u64,
}

impl Default for StateDataSection {
    fn default() -> Self {
        Self {
            data_folder: "./tmp/chain_data/testnet".to_owned(),
            max_cached_heights: 100,
        }
    }
}
