use super::messages::{GetExplorerStakersRequest, GetExplorerStakersResponse};
use super::{NodeContext, NodeError};
use crate::blockchain::Blockchain;
use crate::node::KvStore;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn get_explorer_stakers<B: Blockchain>(
    context: Arc<RwLock<NodeContext<B>>>,
    _req: GetExplorerStakersRequest,
) -> Result<GetExplorerStakersResponse, NodeError> {
    let context = context.read().await;
    let current = context.blockchain.get_stakers()?;
    Ok(GetExplorerStakersResponse {
        current: current.iter().map(|b| b.into()).collect(),
    })
}
