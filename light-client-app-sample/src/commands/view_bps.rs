//! `view-bps` subcommand - to print the block producers data corresponding to a certain epoch.

use crate::light_client::LightClient;
/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::prelude::*;
use abscissa_core::{Command, Runnable};
use borsh::{BorshDeserialize, BorshSerialize};
use near_light_client::near_types::hash::CryptoHash;
use near_light_client::near_types::ValidatorStakeView;
use near_light_client::NearLightClient;

/// `view-bps` subcommand
///
/// The `Parser` proc macro generates an option parser based on the struct
/// definition, and is defined in the `clap` crate. See their documentation
/// for a more comprehensive example:
///
/// <https://docs.rs/clap/>
#[derive(clap::Parser, Command, Debug)]
pub struct ViewBpsCmd {
    /// base58 formatted epoch id
    pub epoch_id: String,
}

impl Runnable for ViewBpsCmd {
    /// Start the application.
    fn run(&self) {
        let light_client = LightClient::new(APP.config().state_data.data_folder.clone());
        status_info!(
            "Info",
            "Latest height of light client: {}",
            light_client.latest_height().unwrap_or(0)
        );
        let bytes = bs58::decode(self.epoch_id.clone()).into_vec().unwrap();
        if let Some(bps) =
            light_client.get_epoch_block_producers(&CryptoHash::try_from(bytes.as_ref()).unwrap())
        {
            status_info!(
                "Info",
                "Bps count of epoch {}: {}",
                self.epoch_id,
                bps.len()
            );
            #[derive(Debug, BorshDeserialize, BorshSerialize)]
            struct BlockProducers(Vec<ValidatorStakeView>);
            status_info!(
                "Info",
                "Bps of epoch {}: {:?}",
                self.epoch_id,
                BlockProducers(bps)
            );
        } else {
            status_err!("Missing bps data of epoch {}.", self.epoch_id);
            return;
        }
    }
}
