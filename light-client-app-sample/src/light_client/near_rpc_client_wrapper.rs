//! Implementation of a wrapper of NEAR JsonRpcClient.
//!

use std::fmt::Debug;

use abscissa_core::Application;
use near_jsonrpc_client::methods::query::RpcQueryRequest;
use near_jsonrpc_client::{methods, JsonRpcClient, MethodCallResult};
use near_jsonrpc_primitives::types::query::QueryResponseKind;
use near_primitives::hash::CryptoHash;
use near_primitives::types::{AccountId, BlockId, Finality, StoreKey};
use near_primitives::views::{BlockView, FinalExecutionOutcomeView, QueryRequest};
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;

use crate::application::APP;

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

    pub(crate) async fn get_next_light_client_block(
        &self,
        last_block_hash: &CryptoHash,
    ) -> anyhow::Result<near_primitives::views::LightClientBlockView> {
        retry(|| async {
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
        })
        .await
    }

    pub(crate) async fn query_broadcast_tx(
        &self,
        method: &methods::broadcast_tx_commit::RpcBroadcastTxCommitRequest,
    ) -> MethodCallResult<
        FinalExecutionOutcomeView,
        near_jsonrpc_primitives::types::transactions::RpcTransactionError,
    > {
        retry(|| async {
            let result = self.rpc_client.call(method).await;
            match &result {
                Ok(response) => {
                    // When user sets logging level to INFO we only print one-liners with submitted
                    // actions and the resulting status. If the level is DEBUG or lower, we print
                    // the entire request and response structures.
                    if tracing::level_enabled!(tracing::Level::DEBUG) {
                        tracing::debug!(
                            target: "workspaces",
                            "Calling RPC method {:?} succeeded with {:?}",
                            method,
                            response
                        );
                    } else {
                        tracing::info!(
                            target: "workspaces",
                            "Submitting transaction with actions {:?} succeeded with status {:?}",
                            method.signed_transaction.transaction.actions,
                            response.status
                        );
                    }
                }
                Err(error) => {
                    tracing::error!(
                        target: "workspaces",
                        "Calling RPC method {:?} resulted in error {:?}",
                        method,
                        error
                    );
                }
            };
            result
        })
        .await
    }

    pub(crate) async fn query<M>(&self, method: &M) -> MethodCallResult<M::Response, M::Error>
    where
        M: methods::RpcMethod + Debug,
        M::Response: Debug,
        M::Error: Debug,
    {
        retry(|| async {
            let result = self.rpc_client.call(method).await;
            tracing::debug!(
                target: "workspaces",
                "Querying RPC with {:?} resulted in {:?}",
                method,
                result
            );
            result
        })
        .await
    }

    pub(crate) async fn view_state_with_proof(
        &self,
        contract_id: AccountId,
        prefix: Option<&[u8]>,
        block_id: Option<BlockId>,
    ) -> anyhow::Result<near_primitives::views::ViewStateResult> {
        retry(|| async {
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
        })
        .await
    }

    pub(crate) async fn view_block(&self, block_id: &Option<BlockId>) -> anyhow::Result<BlockView> {
        retry(|| async {
            let block_reference = block_id
                .clone()
                .map(Into::into)
                .unwrap_or_else(|| Finality::None.into());

            let block_view = self
                .query(&methods::block::RpcBlockRequest { block_reference })
                .await?;

            Ok(block_view)
        })
        .await
    }
}

pub(crate) async fn retry<R, E, T, F>(task: F) -> T::Output
where
    F: FnMut() -> T,
    T: core::future::Future<Output = Result<R, E>>,
{
    // Exponential backoff starting w/ 10ms for maximum retry of 4 times with the following delays:
    //   10, 100, 1000, 10000, 100000 ms
    let retry_strategy = ExponentialBackoff::from_millis(10)
        .map(jitter)
        .take(APP.config().near_rpc.max_retries as usize);

    Retry::spawn(retry_strategy, task).await
}
