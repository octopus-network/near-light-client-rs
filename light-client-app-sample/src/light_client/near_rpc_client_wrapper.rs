//! Implementation of a wrapper of NEAR JsonRpcClient.
//!

use std::fmt::Debug;

use near_jsonrpc_client::methods::light_client_proof::RpcLightClientExecutionProofResponse;
use near_jsonrpc_client::methods::query::RpcQueryRequest;
use near_jsonrpc_client::{methods, JsonRpcClient, MethodCallResult};
use near_jsonrpc_primitives::types::query::QueryResponseKind;
use near_primitives::hash::CryptoHash;
use near_primitives::types::{AccountId, BlockId, Finality, StoreKey, TransactionOrReceiptId};
use near_primitives::views::{BlockView, QueryRequest};
use tokio_retry::strategy::{jitter, ExponentialBackoff, FixedInterval};
use tokio_retry::Retry;

use crate::info_with_time;

enum RetryStrategy {
    ExponentialBackoff,
    FixedInterval,
}

const ERR_INVALID_VARIANT: &str =
    "Incorrect variant retrieved while querying: maybe a bug in RPC code?";

/// A client that wraps around [`JsonRpcClient`], and provides more capabilities such
/// as retry w/ exponential backoff and utility functions for sending transactions.
pub struct NearRpcClientWrapper {
    ///
    pub rpc_addr: String,
    ///
    pub rpc_client: JsonRpcClient,
}

impl NearRpcClientWrapper {
    pub(crate) fn new(rpc_addr: &str) -> Self {
        let connector = JsonRpcClient::new_client();
        let rpc_client = connector.connect(rpc_addr);

        Self {
            rpc_client,
            rpc_addr: rpc_addr.into(),
        }
    }

    pub(crate) async fn query<M>(&self, method: &M) -> MethodCallResult<M::Response, M::Error>
    where
        M: methods::RpcMethod + Debug,
        M::Response: Debug,
        M::Error: Debug,
    {
        retry(
            || async {
                info_with_time!("Try querying {:?} ...", method);
                let result = self.rpc_client.call(method).await;
                tracing::debug!(
                    target: "workspaces",
                    "Querying RPC with {:?} resulted in {:?}",
                    method,
                    result
                );
                result
            },
            RetryStrategy::FixedInterval,
        )
        .await
    }

    pub(crate) async fn get_next_light_client_block(
        &self,
        last_block_hash: &CryptoHash,
    ) -> anyhow::Result<near_primitives::views::LightClientBlockView> {
        retry(
            || async {
                let query_resp = self
                    .query(
                        &methods::next_light_client_block::RpcLightClientNextBlockRequest {
                            last_block_hash: last_block_hash.clone(),
                        },
                    )
                    .await?;
                if query_resp.is_some() {
                    anyhow::Ok(query_resp.unwrap())
                } else {
                    anyhow::bail!("Failed to get next light client block. Response is empty.")
                }
            },
            RetryStrategy::ExponentialBackoff,
        )
        .await
    }

    pub(crate) async fn view_state_with_proof(
        &self,
        contract_id: AccountId,
        prefix: Option<&[u8]>,
        block_id: Option<BlockId>,
    ) -> anyhow::Result<near_primitives::views::ViewStateResult> {
        retry(
            || async {
                let block_reference = block_id
                    .clone()
                    .map(Into::into)
                    .unwrap_or_else(|| Finality::None.into());

                let query_resp = self
                    .query(&RpcQueryRequest {
                        block_reference,
                        request: QueryRequest::ViewState {
                            account_id: contract_id.clone(),
                            prefix: StoreKey::from(prefix.map(Vec::from).unwrap_or_default()),
                            include_proof: true,
                        },
                    })
                    .await?;

                match query_resp.kind {
                    QueryResponseKind::ViewState(state) => anyhow::Ok(state),
                    _ => anyhow::bail!(ERR_INVALID_VARIANT),
                }
            },
            RetryStrategy::ExponentialBackoff,
        )
        .await
    }

    pub(crate) async fn get_light_client_proof(
        &self,
        id: &TransactionOrReceiptId,
        light_client_head: &CryptoHash,
    ) -> anyhow::Result<RpcLightClientExecutionProofResponse> {
        retry(
            || async {
                let query_resp = self
                    .query(
                        &methods::light_client_proof::RpcLightClientExecutionProofRequest {
                            id: id.clone(),
                            light_client_head: light_client_head.clone(),
                        },
                    )
                    .await?;
                anyhow::Ok(query_resp)
            },
            RetryStrategy::ExponentialBackoff,
        )
        .await
    }

    pub(crate) async fn view_block(&self, block_id: &Option<BlockId>) -> anyhow::Result<BlockView> {
        retry(
            || async {
                let block_reference = block_id
                    .clone()
                    .map(Into::into)
                    .unwrap_or_else(|| Finality::None.into());

                let block_view = self
                    .query(&methods::block::RpcBlockRequest { block_reference })
                    .await?;

                Ok(block_view)
            },
            RetryStrategy::ExponentialBackoff,
        )
        .await
    }
}

async fn retry<R, E, T, F>(task: F, strategy: RetryStrategy) -> T::Output
where
    F: FnMut() -> T,
    T: core::future::Future<Output = Result<R, E>>,
{
    match strategy {
        RetryStrategy::ExponentialBackoff => {
            // Exponential backoff starting w/ 10ms for maximum retry of 3 times with the following delays:
            //   100, 10000, 1000000 ms
            let retry_strategy = ExponentialBackoff::from_millis(100).map(jitter).take(3);
            Retry::spawn(retry_strategy, task).await
        }
        RetryStrategy::FixedInterval => {
            let retry_strategy = FixedInterval::from_millis(1000).map(jitter).take(3);
            Retry::spawn(retry_strategy, task).await
        }
    }
}
