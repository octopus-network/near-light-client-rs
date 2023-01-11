# near-light-client-rs

A light client implementation of NEAR protocol, written in Rust language.

The crate `near-light-client` defines all necessary interfaces/functions for NEAR light client and provides default implementation of light client block head validation and contract state validation. It is implemented as `no-std` and tried to keep minimal dependencies. It doesn't include any persistence logic too. This crate can be used in other Rust based applications which need NEAR light client implementation, like `Substrate`.

The crate `light-client-app-sample` provides a basic implementation of a NEAR light client instance, which uses files to store the state data. It's a CLI application based on [abscissa](https://docs.rs/abscissa/0.7.0/abscissa/). It provides the following sample functions:

* Sub-command `start` - to start a NEAR light client instance which will cache a certain range of heights of light client block headers and block producers of epochs in files.
* Sub-command `validate-state` - to validate state data of a contract at a certain height.
* Sub-command `validate-tx` - to validate a certain transaction at latest light client head.
* Sub-command `view-bps` - to print the block producers data corresponding to a certain epoch.
* Sub-command `view-head` - to print the head data at a certain height.
