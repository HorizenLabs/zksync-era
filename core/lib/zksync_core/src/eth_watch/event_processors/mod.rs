use std::fmt;

use zksync_dal::{Connection, Core};
use zksync_types::{web3::types::Log, H256};

use crate::eth_watch::client::{Error, EthClient};

pub mod governance_upgrades;
pub mod nh;
pub mod priority_ops;
pub mod upgrades;

#[async_trait::async_trait]
pub trait EventProcessor: 'static + fmt::Debug + Send + Sync {
    /// Processes given events
    async fn process_events(
        &mut self,
        storage: &mut Connection<'_, Core>,
        client: &dyn EthClient,
        events: Vec<Log>,
    ) -> Result<(), Error>;

    /// Relevant topic which defines what events to be processed
    fn relevant_topic(&self) -> H256;
}
