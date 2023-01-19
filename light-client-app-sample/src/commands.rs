//! LightClientAppSample Subcommands
//!
//! This is where you specify the subcommands of your application.
//!
//! The default application comes with two subcommands:
//!
//! - `start`: launches the application
//! - `--version`: print application version
//!
//! See the `impl Configurable` below for how to specify the path to the
//! application's configuration file.

mod start;
mod verify_membership;
mod verify_non_membership;
mod verify_transaction;
mod view_head;

use self::{
    start::StartCmd, verify_membership::VerifyMembershipCmd,
    verify_non_membership::VerifyNonMembershipCmd, verify_transaction::VerifyTransactionCmd,
    view_head::ViewHeadCmd,
};
use crate::config::LightClientAppSampleConfig;
use abscissa_core::{config::Override, Command, Configurable, FrameworkError, Runnable};
use std::path::PathBuf;

/// LightClientAppSample Configuration Filename
pub const CONFIG_FILE: &str = "light_client_app_sample.toml";

/// LightClientAppSample Subcommands
/// Subcommands need to be listed in an enum.
#[derive(clap::Parser, Command, Debug, Runnable)]
pub enum LightClientAppSampleCmd {
    /// Start an NEAR light instance and keep updating state.
    Start(StartCmd),
    /// View head data at the given height.
    ViewHead(ViewHeadCmd),
    /// Verify the value of a storage key of a NEAR account with proof data.
    VerifyMembership(VerifyMembershipCmd),
    /// Verify that a certain storage key of a NEAR account has NO value with proof data
    /// and optional block height.
    VerifyNonMembership(VerifyNonMembershipCmd),
    /// Verify a certain transaction with latest light client head.
    VerifyTransaction(VerifyTransactionCmd),
}

/// Entry point for the application. It needs to be a struct to allow using subcommands!
#[derive(clap::Parser, Command, Debug)]
#[command(author, about, version)]
pub struct EntryPoint {
    #[command(subcommand)]
    cmd: LightClientAppSampleCmd,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Use the specified config file
    #[arg(short, long)]
    pub config: Option<String>,
}

impl Runnable for EntryPoint {
    fn run(&self) {
        self.cmd.run()
    }
}

/// This trait allows you to define how application configuration is loaded.
impl Configurable<LightClientAppSampleConfig> for EntryPoint {
    /// Location of the configuration file
    fn config_path(&self) -> Option<PathBuf> {
        // Check if the config file exists, and if it does not, ignore it.
        // If you'd like for a missing configuration file to be a hard error
        // instead, always return `Some(CONFIG_FILE)` here.
        let filename = self
            .config
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| CONFIG_FILE.into());

        if filename.exists() {
            Some(filename)
        } else {
            None
        }
    }

    /// Apply changes to the config after it's been loaded, e.g. overriding
    /// values in a config file using command-line options.
    ///
    /// This can be safely deleted if you don't want to override config
    /// settings from command-line options.
    fn process_config(
        &self,
        config: LightClientAppSampleConfig,
    ) -> Result<LightClientAppSampleConfig, FrameworkError> {
        match &self.cmd {
            LightClientAppSampleCmd::Start(cmd) => cmd.override_config(config),
            //
            // If you don't need special overrides for some
            // subcommands, you can just use a catch all
            _ => Ok(config),
        }
    }
}
