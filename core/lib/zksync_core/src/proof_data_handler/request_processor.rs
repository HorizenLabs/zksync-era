use std::sync::Arc;

use axum::{
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use crypto_codegen::serialize_proof;
use hex::encode;
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::Keypair;
use zksync_config::configs::ProofDataHandlerConfig;
use zksync_dal::{ConnectionPool, Core, CoreDal, SqlxError};
use zksync_object_store::{ObjectStore, ObjectStoreError};
use zksync_prover_interface::{
    api::{
        ProofGenerationData, ProofGenerationDataRequest, ProofGenerationDataResponse,
        SubmitProofRequest, SubmitProofResponse,
    },
    outputs::L1BatchProofForL1,
};
use zksync_types::{
    basic_fri_types::Eip4844Blobs, commitment::serialize_commitments, web3::signing::keccak256,
    L1BatchNumber, H256,
};
use zksync_utils::{u256_to_bytes_be, u256_to_h256};

#[derive(Clone)]
pub struct NhClient {
    client: OnlineClient<PolkadotConfig>,
    kp: Keypair,
}

impl NhClient {
    pub fn new(client: OnlineClient<PolkadotConfig>, kp: Keypair) -> Self {
        Self { client, kp }
    }

    async fn submit_proof(&self, raw_proof: [u8; 1440]) -> Result<(u64, H256), subxt::Error> {
        let submit_proof_tx = nh::tx().settlement_zksync_pallet().submit_proof(raw_proof);

        // Submit the submitProof extrinsic, and wait for it to be successful
        // and in a finalized block. We get back the extrinsic events if all is well.
        let events = self
            .client
            .tx()
            .sign_and_submit_then_watch_default(&submit_proof_tx, &self.kp)
            .await?
            .wait_for_finalized_success()
            .await?;

        // Find a Transfer event and print it.
        let transfer_event = events.find_first::<nh::poe::events::NewElement>()?.unwrap();
        println!("New Element success: {transfer_event:?}");
        Ok((transfer_event.attestation_id, transfer_event.value))
    }
}

#[derive(Clone)]
pub(crate) struct RequestProcessor {
    blob_store: Arc<dyn ObjectStore>,
    pool: ConnectionPool<Core>,
    config: ProofDataHandlerConfig,
    nh_client: Option<NhClient>,
}

pub(crate) enum RequestProcessorError {
    ObjectStore(ObjectStoreError),
    Sqlx(SqlxError),
}

#[subxt::subxt(runtime_metadata_path = "../../../etc/nh/metadata.scale")]
pub mod nh {}

impl IntoResponse for RequestProcessorError {
    fn into_response(self) -> Response {
        let (status_code, message) = match self {
            RequestProcessorError::ObjectStore(err) => {
                tracing::error!("GCS error: {:?}", err);
                (
                    StatusCode::BAD_GATEWAY,
                    "Failed fetching/saving from GCS".to_owned(),
                )
            }
            RequestProcessorError::Sqlx(err) => {
                tracing::error!("Sqlx error: {:?}", err);
                match err {
                    SqlxError::RowNotFound => {
                        (StatusCode::NOT_FOUND, "Non existing L1 batch".to_owned())
                    }
                    _ => (
                        StatusCode::BAD_GATEWAY,
                        "Failed fetching/saving from db".to_owned(),
                    ),
                }
            }
        };
        (status_code, message).into_response()
    }
}

impl RequestProcessor {
    pub(crate) fn new(
        blob_store: Arc<dyn ObjectStore>,
        pool: ConnectionPool<Core>,
        config: ProofDataHandlerConfig,
        nh_client: Option<NhClient>,
    ) -> Self {
        Self {
            blob_store,
            pool,
            config,
            nh_client,
        }
    }

    pub(crate) async fn get_proof_generation_data(
        &self,
        request: Json<ProofGenerationDataRequest>,
    ) -> Result<Json<ProofGenerationDataResponse>, RequestProcessorError> {
        tracing::info!("Received request for proof generation data: {:?}", request);

        let l1_batch_number_result = self
            .pool
            .connection()
            .await
            .unwrap()
            .proof_generation_dal()
            .get_next_block_to_be_proven(self.config.proof_generation_timeout())
            .await;

        let l1_batch_number = match l1_batch_number_result {
            Some(number) => number,
            None => return Ok(Json(ProofGenerationDataResponse::Success(None))), // no batches pending to be proven
        };

        let blob = self
            .blob_store
            .get(l1_batch_number)
            .await
            .map_err(RequestProcessorError::ObjectStore)?;

        let header = self
            .pool
            .connection()
            .await
            .unwrap()
            .blocks_dal()
            .get_l1_batch_header(l1_batch_number)
            .await
            .unwrap()
            .expect(&format!("Missing header for {}", l1_batch_number));

        let protocol_version_id = header.protocol_version.unwrap();
        let l1_verifier_config = self
            .pool
            .connection()
            .await
            .unwrap()
            .protocol_versions_dal()
            .l1_verifier_config_for_version(protocol_version_id)
            .await
            .expect(&format!(
                "Missing l1 verifier info for protocol version {protocol_version_id:?}",
            ));

        let storage_batch = self
            .pool
            .connection()
            .await
            .unwrap()
            .blocks_dal()
            .get_storage_l1_batch(l1_batch_number)
            .await
            .unwrap()
            .unwrap();

        let eip_4844_blobs: Eip4844Blobs = storage_batch
            .pubdata_input
            .expect(&format!(
                "expected pubdata, but it is not available for batch {l1_batch_number:?}"
            ))
            .into();

        let proof_gen_data = ProofGenerationData {
            l1_batch_number,
            data: blob,
            protocol_version_id,
            l1_verifier_config,
            eip_4844_blobs,
        };
        Ok(Json(ProofGenerationDataResponse::Success(Some(
            proof_gen_data,
        ))))
    }

    pub(crate) async fn submit_proof(
        &self,
        Path(l1_batch_number): Path<u32>,
        Json(payload): Json<SubmitProofRequest>,
    ) -> Result<Json<SubmitProofResponse>, RequestProcessorError> {
        println!("Submit proof!");
        tracing::info!("Received proof for block number: {:?}", l1_batch_number);
        let l1_batch_number = L1BatchNumber(l1_batch_number);
        match payload {
            SubmitProofRequest::Proof(proof) => {
                let (_input, _p) = serialize_proof(&proof.scheduler_proof);
                let blob_url = self
                    .blob_store
                    .put(l1_batch_number, &*proof)
                    .await
                    .map_err(RequestProcessorError::ObjectStore)?;

                let system_logs_hash_from_prover =
                    H256::from_slice(&proof.aggregation_result_coords[0]);
                let state_diff_hash_from_prover =
                    H256::from_slice(&proof.aggregation_result_coords[1]);
                let bootloader_heap_initial_content_from_prover =
                    H256::from_slice(&proof.aggregation_result_coords[2]);
                let events_queue_state_from_prover =
                    H256::from_slice(&proof.aggregation_result_coords[3]);

                let mut storage = self.pool.connection().await.unwrap();

                let l1_batch = storage
                    .blocks_dal()
                    .get_l1_batch_metadata(l1_batch_number)
                    .await
                    .unwrap()
                    .expect("Proved block without metadata");

                let is_pre_boojum = l1_batch
                    .header
                    .protocol_version
                    .map(|v| v.is_pre_boojum())
                    .unwrap_or(true);
                if !is_pre_boojum {
                    let events_queue_state = l1_batch
                        .metadata
                        .events_queue_commitment
                        .expect("No events_queue_commitment");
                    let bootloader_heap_initial_content = l1_batch
                        .metadata
                        .bootloader_initial_content_commitment
                        .expect("No bootloader_initial_content_commitment");

                    if events_queue_state != events_queue_state_from_prover
                        || bootloader_heap_initial_content
                            != bootloader_heap_initial_content_from_prover
                    {
                        let server_values = format!("events_queue_state = {events_queue_state}, bootloader_heap_initial_content = {bootloader_heap_initial_content}");
                        let prover_values = format!("events_queue_state = {events_queue_state_from_prover}, bootloader_heap_initial_content = {bootloader_heap_initial_content_from_prover}");
                        panic!(
                            "Auxilary output doesn't match, server values: {} prover values: {}",
                            server_values, prover_values
                        );
                    }
                }

                let system_logs = serialize_commitments(&l1_batch.header.system_logs);
                let system_logs_hash = H256(keccak256(&system_logs));

                if !is_pre_boojum {
                    let state_diff_hash = l1_batch
                        .header
                        .system_logs
                        .into_iter()
                        .find(|elem| elem.0.key == u256_to_h256(2.into()))
                        .expect("No state diff hash key")
                        .0
                        .value;

                    if state_diff_hash != state_diff_hash_from_prover
                        || system_logs_hash != system_logs_hash_from_prover
                    {
                        let server_values = format!("system_logs_hash = {system_logs_hash}, state_diff_hash = {state_diff_hash}");
                        let prover_values = format!("system_logs_hash = {system_logs_hash_from_prover}, state_diff_hash = {state_diff_hash_from_prover}");
                        panic!(
                            "Auxilary output doesn't match, server values: {} prover values: {}",
                            server_values, prover_values
                        );
                    }
                }
                storage
                    .proof_generation_dal()
                    .save_proof_artifacts_metadata(l1_batch_number, &blob_url)
                    .await
                    .map_err(RequestProcessorError::Sqlx)?;
            }
            SubmitProofRequest::SkippedProofGeneration => {
                println!(
                    "SubmitProof --> SkippedProofGeneration {}",
                    l1_batch_number.0
                );

                // Build a submitProof extrinsic.

                //Mock proof
                let data = include_bytes!("../../../../../etc/nh/l1_batch_proof_1.bin");
                let parsed: L1BatchProofForL1 = bincode::deserialize(data.as_slice()).unwrap();

                let (input, p) = serialize_proof(&parsed.scheduler_proof);
                let proof_bytes = p.iter().flat_map(u256_to_bytes_be).collect::<Vec<u8>>();
                let pi_bytes = input.iter().flat_map(u256_to_bytes_be).collect::<Vec<u8>>();

                println!("PROOF {:?}", encode(proof_bytes.clone()));
                println!("PI {:?}", encode(pi_bytes.clone()));

                let raw_proof: [u8; 1440] = [proof_bytes, pi_bytes].concat().as_slice()[0..1440]
                    .try_into()
                    .unwrap();

                let (attestation_id, value) = self
                    .nh_client
                    .as_ref()
                    .unwrap()
                    .submit_proof(raw_proof)
                    .await
                    .unwrap();

                self.pool
                    .connection()
                    .await
                    .unwrap()
                    .proof_generation_dal()
                    .mark_proof_generation_job_as_skipped(l1_batch_number, attestation_id, value)
                    .await
                    .map_err(RequestProcessorError::Sqlx)?;
            }
        }

        Ok(Json(SubmitProofResponse::Success))
    }
}
