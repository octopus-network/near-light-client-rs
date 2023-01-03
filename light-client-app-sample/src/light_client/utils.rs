//! Some util functions related to NEAR light client.
//!

use abscissa_core::status_info;
use near_light_client::{
    near_types::{
        hash::CryptoHash,
        signature::{ED25519PublicKey, PublicKey, Signature},
        BlockHeaderInnerLiteView, LightClientBlockView, ValidatorStakeView, ValidatorStakeViewV1,
    },
    LightClientBlockViewExt,
};
use near_primitives::views::BlockView;

/// Produce `LightClientBlockViewExt` by `LightClientBlockView` and `BlockView`.
pub fn produce_light_client_block_view(
    view_from_near: &near_primitives::views::LightClientBlockView,
    block_view: &BlockView,
) -> LightClientBlockViewExt {
    print_light_client_block_view(&view_from_near);
    print_block_view(&block_view);
    assert!(
        view_from_near.inner_lite.height == block_view.header.height,
        "Not same height of light client block view and block view."
    );
    LightClientBlockViewExt {
        light_client_block_view: LightClientBlockView {
            prev_block_hash: CryptoHash(view_from_near.prev_block_hash.0),
            next_block_inner_hash: CryptoHash(view_from_near.next_block_inner_hash.0),
            inner_lite: BlockHeaderInnerLiteView {
                height: view_from_near.inner_lite.height,
                epoch_id: CryptoHash(view_from_near.inner_lite.epoch_id.0),
                next_epoch_id: CryptoHash(view_from_near.inner_lite.next_epoch_id.0),
                prev_state_root: CryptoHash(view_from_near.inner_lite.prev_state_root.0),
                outcome_root: CryptoHash(view_from_near.inner_lite.outcome_root.0),
                timestamp: view_from_near.inner_lite.timestamp,
                timestamp_nanosec: view_from_near.inner_lite.timestamp_nanosec,
                next_bp_hash: CryptoHash(view_from_near.inner_lite.next_bp_hash.0),
                block_merkle_root: CryptoHash(view_from_near.inner_lite.block_merkle_root.0),
            },
            inner_rest_hash: CryptoHash(view_from_near.inner_rest_hash.0),
            next_bps: Some(
                view_from_near
                    .next_bps
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
            approvals_after_next: view_from_near
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

/// Print general info of `LightClientBlockView` with macro `status_info`.
pub fn print_light_client_block_view(view: &near_primitives::views::LightClientBlockView) {
    status_info!(
        "Updating",
        "LightClientBlockView: {{ prev_block_hash: {}, height: {}, prev_state_root: {}, epoch_id: {}, next_epoch_id: {} }}",
        view.prev_block_hash,
        view.inner_lite.height,
        view.inner_lite.prev_state_root,
        view.inner_lite.epoch_id,
        view.inner_lite.next_epoch_id
    );
}

/// Print general info of `BlockView` with macro `status_info`.
pub fn print_block_view(view: &BlockView) {
    status_info!(
        "Updating",
        "BlockView: {{ height: {}, prev_height: {:?}, prev_state_root: {}, epoch_id: {}, next_epoch_id: {}, hash: {}, prev_hash: {} }}",
        view.header.height,
        view.header.prev_height,
        view.header.prev_state_root,
        view.header.epoch_id,
        view.header.next_epoch_id,
        view.header.hash,
        view.header.prev_hash
    );
}
