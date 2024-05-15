use std::convert::TryFrom;

use zksync_contracts::nh_contract;
use zksync_dal::{Connection, Core, CoreDal};
use zksync_types::{l1::NHAttestation, Log, H256};

use crate::eth_watch::{
    client::{Error, EthClient},
    event_processors::EventProcessor,
};

pub(crate) const ATTESTATION_POSTED_SIGNATURE: H256 = H256([
    230, 78, 18, 226, 223, 214, 132, 174, 145, 191, 26, 108, 82, 251, 242, 175, 105, 134, 219, 89,
    39, 210, 63, 91, 100, 62, 13, 80, 247, 222, 72, 120,
]);

/// Responsible for saving new priority L1 transactions to the database.
#[derive(Debug)]
pub struct NHEventProcessor {
    nh_attestation_signature: H256,
}

impl NHEventProcessor {
    pub fn new() -> Self {
        Self {
            nh_attestation_signature: nh_contract()
                .event("AttestationPosted")
                .expect("AttestationPosted event is missing in abi")
                .signature(),
        }
    }
}

#[async_trait::async_trait]
impl EventProcessor for NHEventProcessor {
    async fn process_events(
        &mut self,
        storage: &mut Connection<'_, Core>,
        _client: &dyn EthClient,
        events: Vec<Log>,
    ) -> Result<(), Error> {
        let mut nh_ops = Vec::new();
        // Assuming we only have one kind of event
        for event in events
            .into_iter()
            .filter(|event| event.topics[0] == ATTESTATION_POSTED_SIGNATURE)
        {
            let op = NHAttestation::try_from(event)
                .map_err(|err| Error::LogParse(format!("{}", err)))?;
            tracing::info!("Found new NH attestation: {op}");
            nh_ops.push(op);
        }

        if nh_ops.is_empty() {
            return Ok(());
        }

        let new_attestations: Vec<_> = nh_ops.into_iter().collect();
        if new_attestations.is_empty() {
            return Ok(());
        }

        for new_attestation in new_attestations {
            storage
                .nh_dal()
                .insert_nh_attestation(&new_attestation)
                .await;
        }
        Ok(())
    }

    fn relevant_topic(&self) -> H256 {
        self.nh_attestation_signature
    }
}
