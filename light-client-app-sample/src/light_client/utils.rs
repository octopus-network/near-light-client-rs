//! Some util functions related to NEAR light client.
//!

use abscissa_core::status_info;
use near_light_client::{
    near_types::{
        hash::CryptoHash,
        signature::{ED25519PublicKey, PublicKey, Signature},
        BlockHeaderInnerLiteView, LightClientBlockLiteView, LightClientBlockView,
        ValidatorStakeView, ValidatorStakeViewV1,
    },
    types::{ConsensusState, Header},
};
use near_primitives::views::BlockView;

/// Produce `BlockHeaderInnerLiteView` by its NEAR version
pub fn produce_block_header_inner_light_view(
    view: &near_primitives::views::BlockHeaderInnerLiteView,
) -> BlockHeaderInnerLiteView {
    BlockHeaderInnerLiteView {
        height: view.height,
        epoch_id: CryptoHash(view.epoch_id.0),
        next_epoch_id: CryptoHash(view.next_epoch_id.0),
        prev_state_root: CryptoHash(view.prev_state_root.0),
        outcome_root: CryptoHash(view.outcome_root.0),
        timestamp: view.timestamp,
        timestamp_nanosec: view.timestamp_nanosec,
        next_bp_hash: CryptoHash(view.next_bp_hash.0),
        block_merkle_root: CryptoHash(view.block_merkle_root.0),
    }
}

/// Produce `Header` by NEAR version of `LightClientBlockView` and `BlockView`.
pub fn produce_light_client_block_view(
    view: &near_primitives::views::LightClientBlockView,
    block_view: &BlockView,
) -> Header {
    assert!(
        view.inner_lite.height == block_view.header.height,
        "Not same height of light client block view and block view."
    );
    Header {
        light_client_block_view: LightClientBlockView {
            prev_block_hash: CryptoHash(view.prev_block_hash.0),
            next_block_inner_hash: CryptoHash(view.next_block_inner_hash.0),
            inner_lite: produce_block_header_inner_light_view(&view.inner_lite),
            inner_rest_hash: CryptoHash(view.inner_rest_hash.0),
            next_bps: Some(
                view.next_bps
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|f| match f {
                        near_primitives::views::validator_stake_view::ValidatorStakeView::V1(v) => {
                            ValidatorStakeView::V1(ValidatorStakeViewV1 {
                                account_id: v.account_id.to_string(),
                                public_key: match &v.public_key {
                                    near_crypto::PublicKey::ED25519(data) => {
                                        PublicKey::ED25519(ED25519PublicKey(data.clone().0))
                                    }
                                    _ => panic!("Unsupported publickey in next block producers."),
                                },
                                stake: v.stake,
                            })
                        }
                    })
                    .collect(),
            ),
            approvals_after_next: view
                .approvals_after_next
                .iter()
                .map(|f| {
                    f.as_ref().map(|s| match s {
                        near_crypto::Signature::ED25519(data) => Signature::ED25519(data.clone()),
                        _ => panic!("Unsupported signature in approvals after next."),
                    })
                })
                .collect(),
        },
        prev_state_root_of_chunks: block_view
            .chunks
            .iter()
            .map(|header| CryptoHash(header.prev_state_root.0))
            .collect(),
    }
}

/// Producer `LightClientBlockLiteView` by its NEAR version
pub fn produce_light_client_block_lite_view(
    view: &near_primitives::views::LightClientBlockLiteView,
) -> LightClientBlockLiteView {
    LightClientBlockLiteView {
        inner_lite: produce_block_header_inner_light_view(&view.inner_lite),
        inner_rest_hash: CryptoHash(view.inner_rest_hash.0),
        prev_block_hash: CryptoHash(view.prev_block_hash.0),
    }
}

/// Print general info of `LightClientBlockView` with macro `status_info`.
pub fn print_light_client_consensus_state(view: &ConsensusState) {
    status_info!(
        "Info",
        "ConsensusState: {{ prev_block_hash: {}, height: {}, prev_state_root: {}, epoch_id: {}, next_epoch_id: {}, signature_count: {}, current_bps_count: {}, next_bps_count: {} }}",
        view.header.light_client_block_view.prev_block_hash,
        view.header.height(),
        view.header.light_client_block_view.inner_lite.prev_state_root,
        view.header.epoch_id(),
        view.header.next_epoch_id(),
        view.header.light_client_block_view.approvals_after_next.len(),
        view.current_bps.as_ref().map_or(0, |bps| bps.len()),
        view.header.light_client_block_view.next_bps.as_ref().map_or(0, |bps| bps.len()),
    );
}

/// Print general info of `BlockView` with macro `status_info`.
pub fn print_block_view(view: &BlockView) {
    status_info!(
        "Info",
        "BlockView: {{ height: {}, prev_height: {:?}, prev_state_root: {}, epoch_id: {}, next_epoch_id: {}, hash: {}, prev_hash: {}, prev_state_root_of_chunks: {:?} }}",
        view.header.height,
        view.header.prev_height,
        view.header.prev_state_root,
        view.header.epoch_id,
        view.header.next_epoch_id,
        view.header.hash,
        view.header.prev_hash,
        view.chunks.iter().map(|h| h.prev_state_root).collect::<Vec<near_primitives::hash::CryptoHash>>(),
    );
}
