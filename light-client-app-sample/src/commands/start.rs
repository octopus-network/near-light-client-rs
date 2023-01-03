//! `start` subcommand - example of how to write a subcommand

use std::time::Duration;

use crate::config::LightClientAppSampleConfig;
use crate::light_client::utils::produce_light_client_block_view;
use crate::light_client::{near_rpc_client_wrapper::NearRpcClientWrapper, LightClient};
/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::prelude::*;
use abscissa_core::{config, Command, FrameworkError, Runnable};
use near_light_client::{LightClientBlockViewExt, NearLightClient};
use near_primitives::types::BlockId;

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
    let mut last_block_hash = match light_client.latest_head() {
        Some(head) => {
            near_primitives::hash::CryptoHash(head.light_client_block_view.prev_block_hash.0)
        }
        None => {
            let latest_block = rpc_client
                .view_block(
                    &light_client
                        .latest_height()
                        .map(|height| BlockId::Height(height + 1)),
                )
                .await
                .expect("Failed to get latest block.");
            latest_block.header.prev_hash
        }
    };
    loop {
        let light_client_block_view = rpc_client
            .get_next_light_client_block(&last_block_hash)
            .await
            .expect("Failed to get next light client block.");
        if light_client_block_view.prev_block_hash == last_block_hash {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        }
        //
        let head = get_light_client_block_view_ext(&light_client_block_view, &rpc_client).await;
        if light_client
            .epoch_block_producers(&head.light_client_block_view.inner_lite.epoch_id)
            .is_none()
        {
            light_client.update_head(head);
        } else {
            light_client
                .validate_and_update_head(head)
                .expect("Failed to update state of NEAR light client.");
        }
        last_block_hash = light_client_block_view.prev_block_hash;
        //
        while light_client.cached_heights().len()
            > APP.config().state_data.max_cached_heights as usize
        {
            light_client.remove_oldest_head();
        }
    }
}

async fn get_light_client_block_view_ext(
    light_client_block_view: &near_primitives::views::LightClientBlockView,
    rpc_client: &NearRpcClientWrapper,
) -> LightClientBlockViewExt {
    let block_view = rpc_client
        .view_block(&Some(BlockId::Height(
            light_client_block_view.inner_lite.height,
        )))
        .await
        .expect(
            format!(
                "Failed to get block view at height {}.",
                light_client_block_view.inner_lite.height
            )
            .as_str(),
        );
    produce_light_client_block_view(light_client_block_view, &block_view)
}
