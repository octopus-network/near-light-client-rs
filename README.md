# near-light-client-rs

A light client implementation of NEAR protocol, written in Rust language.

## Crate `near-light-client`

This crate defines a minimal interface for NEAR light client and provides default implementation of light client header verification, on-chain state verification and transaction/receipt verification.

This crate is implemented as `no-std` and tries to keep minimal dependencies. It doesn't include any implementation for data persistence too.

This crate can be used in other Rust based applications which need basic NEAR light client implementation, like `Substrate` or IBC implementations.

## Crate `light-client-app-sample`

This crate provides a basic implementation of a NEAR light client instance, which uses files to store the state data. It's a CLI application based on [abscissa](https://docs.rs/abscissa/0.7.0/abscissa/). It provides the following sample functions:

* Sub-command `start` - to start a NEAR light client instance which will cache a certain count of consensus states in files.
* Sub-command `verify-membership` - to verify the value of a certain storage key of a NEAR account with proof data and optional block height.
* Sub-command `verify-non-membership` - to verify that a certain storage key of a NEAR account has NO value with proof data and optional block height.
* Sub-command `verify-transaction` - to verify a certain transaction with the latest light client head.
* Sub-command `view-head` - to print the head data at a certain height.

The configuration file for the CLI is [here](light-client-app-sample/light-client-app-sample.toml).
