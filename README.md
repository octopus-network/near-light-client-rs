# near-light-client-rs

A light client implementation of NEAR protocol, written in Rust language.

## Crate `near-light-client`

This crate defines all necessary interfaces/functions for NEAR light client and provides default implementation of light client block header verification, on-chain state verification and transaction/receipt verification. It is implemented as `no-std` and tried to keep minimal dependencies. It doesn't include any persistence logic too. This crate can be used in other Rust based applications which need NEAR light client implementation, like `Substrate`.

The interfaces/functions in this crate are designed based on the [IBC specification](https://github.com/cosmos/ibc) (ics-012-near-client), making it easier to be integrated into the implementation of IBC/TAO as a client component.

## Crate `light-client-app-sample`

This crate provides a basic implementation of a NEAR light client instance, which uses files to store the state data. It's a CLI application based on [abscissa](https://docs.rs/abscissa/0.7.0/abscissa/). It provides the following sample functions:

* Sub-command `start` - to start a NEAR light client instance which will cache a certain range of heights of light client block headers and block producers of epochs in files.
* Sub-command `validate-state` - to validate state data of a contract at a certain height.
* Sub-command `validate-tx` - to validate a certain transaction at latest light client head.
* Sub-command `view-head` - to print the head data at a certain height.
