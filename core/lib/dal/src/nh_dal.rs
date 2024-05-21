use bigdecimal::BigDecimal;
use zksync_basic_types::H256;
use zksync_db_connection::connection::Connection;
use zksync_types::{l1::NHAttestation, L1BatchNumber};
use zksync_utils::{bigdecimal_to_u256, u256_to_big_decimal};

use crate::Core;

#[derive(Debug)]
pub struct NewHorizenDal<'a, 'c> {
    pub(crate) storage: &'a mut Connection<'c, Core>,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct StorageNHAttestation {
    pub attestation_id: BigDecimal,
    pub attestation: Vec<u8>,
}

impl NewHorizenDal<'_, '_> {
    pub async fn insert_nh_attestation(&mut self, nh_attestation: &NHAttestation) {
        sqlx::query!(
            r#"
            INSERT INTO
                new_horizen_attestation (attestation_id, attestation)
            VALUES
                ($1, $2)
            ON CONFLICT (attestation_id) DO NOTHING
            "#,
            u256_to_big_decimal(nh_attestation.attestation_id),
            nh_attestation.proofs_attestation.as_bytes(),
        )
        .execute(self.storage.conn())
        .await
        .unwrap();
    }

    pub async fn get_nh_attestation_from_batch_number(
        &mut self,
        block_number: L1BatchNumber,
    ) -> Option<NHAttestation> {
        let result = sqlx::query_as!(
            StorageNHAttestation,
            r#"
            SELECT
                attestation_id,
                attestation
            FROM
                new_horizen_attestation
            WHERE
                attestation_id = (
                    SELECT
                        attestation_id
                    FROM
                        proof_generation_details
                    WHERE
                        l1_batch_number = $1
                )
            "#,
            i64::from(block_number.0),
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap();

        let nh_attestation: Option<NHAttestation> = match result {
            Some(attestation) => {
                let attestation_id = bigdecimal_to_u256(attestation.attestation_id);
                let proofs_attestation = H256::from_slice(&attestation.attestation);
                Some(NHAttestation {
                    attestation_id,
                    proofs_attestation,
                })
            }
            _ => None,
        };

        nh_attestation
    }
}
