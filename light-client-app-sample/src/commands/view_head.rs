//! `start` subcommand - example of how to write a subcommand

use crate::light_client::LightClient;
/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::prelude::*;
use abscissa_core::{Command, Runnable};
use near_light_client::NearLightClient;

/// `start` subcommand
///
/// The `Parser` proc macro generates an option parser based on the struct
/// definition, and is defined in the `clap` crate. See their documentation
/// for a more comprehensive example:
///
/// <https://docs.rs/clap/>
#[derive(clap::Parser, Command, Debug)]
pub struct ViewHeadCmd {
    pub height: Option<u64>,
    pub with_detail: Option<bool>,
}

impl Runnable for ViewHeadCmd {
    /// Start the application.
    fn run(&self) {
        let light_client = LightClient::new(APP.config().state_data.data_folder.clone());
        status_info!(
            "Info",
            "Latest height of light client: {}",
            light_client.latest_height().unwrap_or(0)
        );
        let height = match self.height {
            Some(height) => height,
            None => match light_client.latest_height() {
                Some(height) => height,
                None => panic!("No head data in client."),
            },
        };
        if let Some(head) = light_client.get_head_at(height) {
            if self.with_detail.map_or(false, |w| w) {
                status_info!("Info", "Head data at height {}: {:?}", height, head);
            } else {
                status_info!(
                    "Info",
                    "Head data at height {}: {{ prev_block_hash: {}, height: {}, prev_state_root: {}, epoch_id: {}, next_epoch_id: {} }}",
                    height,
                    head.light_client_block_view.prev_block_hash,
                    head.light_client_block_view.inner_lite.height,
                    head.light_client_block_view.inner_lite.prev_state_root,
                    head.light_client_block_view.inner_lite.epoch_id,
                    head.light_client_block_view.inner_lite.next_epoch_id
                );
            }
        } else {
            status_err!("Missing head data at height {}.", height);
            return;
        }
    }
}
