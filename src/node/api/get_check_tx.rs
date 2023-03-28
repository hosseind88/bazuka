use super::messages::{CheckTransactionRequest, CheckTransactionResponse};
use super::{NodeContext, NodeError};
use crate::blockchain::Blockchain;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn get_check_tx<B: Blockchain>(
    context: Arc<RwLock<NodeContext<B>>>,
    req: CheckTransactionRequest,
) -> Result<CheckTransactionResponse, NodeError> {
    let context = context.read().await;
    let error = context
        .blockchain
        .check_tx(&req.tx_delta.tx)
        .err()
        .map(|e| e.to_string());

    Ok(CheckTransactionResponse { error })
}
