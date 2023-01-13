//! `start` subcommand - start an instance of NEAR light client.

use crate::config::LightClientAppSampleConfig;
use crate::light_client::utils::produce_light_client_block_view;
use crate::light_client::{near_rpc_client_wrapper::NearRpcClientWrapper, LightClient};
/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::prelude::*;
use abscissa_core::{config, Command, FrameworkError, Runnable};
use near_light_client::types::ConsensusState;
use near_light_client::BasicNearLightClient;
use near_primitives::types::BlockId;
use near_primitives::views::BlockView;

/// `start` subcommand
///
/// The `Parser` proc macro generates an option parser based on the struct
/// definition, and is defined in the `clap` crate. See their documentation
/// for a more comprehensive example:
///
/// <https://docs.rs/clap/>
#[derive(clap::Parser, Command, Debug)]
pub struct StartCmd {}

impl Runnable for StartCmd {
    /// Start the application.
    fn run(&self) {
        abscissa_tokio::run(&APP, start_light_client())
            .expect("Failed to start NEAR light client.");
    }
}

impl config::Override<LightClientAppSampleConfig> for StartCmd {
    // Process the given command line options, overriding settings from
    // a configuration file using explicit flags taken from command-line
    // arguments.
    fn override_config(
        &self,
        config: LightClientAppSampleConfig,
    ) -> Result<LightClientAppSampleConfig, FrameworkError> {
        Ok(config)
    }
}

async fn start_light_client() {
    let rpc_client = NearRpcClientWrapper::new(APP.config().near_rpc.rpc_endpoint.as_str());
    let mut light_client = LightClient::new(APP.config().state_data.data_folder.clone());
    //
    // Keep updating state and save state to file
    //
    let mut block_view = get_block(&rpc_client, &Some(light_client.latest_height())).await;
    loop {
        let light_client_block_view = rpc_client
            .get_next_light_client_block(&block_view.header.hash)
            .await
            .expect("Failed to get next light client block.");
        block_view = get_block(
            &rpc_client,
            &Some(light_client_block_view.inner_lite.height),
        )
        .await;
        let header = produce_light_client_block_view(&light_client_block_view, &block_view);
        if let Some(latest_head) = light_client.get_consensus_state(&light_client.latest_height()) {
            let current_bps = latest_head.get_block_producers_of(&header.epoch_id());
            if current_bps.is_some() {
                if let Err(err) = light_client.verify_header(&header) {
                    status_err!(
                        "Failed to validate state at height {}: {:?}",
                        header.height(),
                        err
                    );
                    light_client.save_failed_head(ConsensusState {
                        current_bps,
                        header,
                    });
                    break;
                } else {
                    light_client.set_consensus_state(
                        &header.height(),
                        ConsensusState {
                            current_bps,
                            header,
                        },
                    );
                }
            } else {
                light_client.set_consensus_state(
                    &header.height(),
                    ConsensusState {
                        current_bps: None,
                        header,
                    },
                );
            }
        } else {
            light_client.set_consensus_state(
                &header.height(),
                ConsensusState {
                    current_bps: None,
                    header,
                },
            );
        }
        //
        while light_client.cached_heights().len()
            > APP.config().state_data.max_cached_heights as usize
        {
            light_client.remove_oldest_head();
        }
    }
}

async fn get_block(rpc_client: &NearRpcClientWrapper, height: &Option<u64>) -> BlockView {
    rpc_client
        .view_block(&height.map(|height| BlockId::Height(height)))
        .await
        .expect(format!("Failed to get block at height {:?}.", height).as_str())
}
