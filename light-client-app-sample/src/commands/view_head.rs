//! `view-head` subcommand - to print the head data at a certain height.

use crate::light_client::{utils::print_light_client_consensus_state, LightClient};
/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::prelude::*;
use abscissa_core::{Command, Runnable};
use near_light_client::BasicNearLightClient;

/// `view-head` subcommand
///
/// The `Parser` proc macro generates an option parser based on the struct
/// definition, and is defined in the `clap` crate. See their documentation
/// for a more comprehensive example:
///
/// <https://docs.rs/clap/>
#[derive(clap::Parser, Command, Debug)]
pub struct ViewHeadCmd {
    pub with_detail: Option<bool>,
    pub height: Option<u64>,
}

impl Runnable for ViewHeadCmd {
    /// Start the application.
    fn run(&self) {
        let light_client = LightClient::new(APP.config().state_data.data_folder.clone());
        status_info!(
            "Info",
            "Latest height of light client: {}",
            light_client.latest_height()
        );
        let height = match self.height {
            Some(height) => height,
            None => light_client.latest_height(),
        };
        if let Some(head) = light_client.get_consensus_state(&height) {
            if self.with_detail.map_or(false, |w| w) {
                status_info!("Info", "Head data at height {}: {:?}", height, head);
            } else {
                print_light_client_consensus_state(&head);
            }
        } else {
            status_err!("Missing head data at height {}.", height);
            return;
        }
    }
}
