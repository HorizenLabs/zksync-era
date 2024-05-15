CREATE TABLE new_horizen_attestation
(
    attestation_id          NUMERIC(80) NOT NULL PRIMARY KEY,
    attestation             BYTEA NOT NULL
);

ALTER TABLE proof_generation_details ADD attestation_id NUMERIC(80);
ALTER TABLE proof_generation_details ADD attestation_element BYTEA;