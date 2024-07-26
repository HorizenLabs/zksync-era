use ethereum_types::U256;
use franklin_crypto::bellman::{
    self,
    pairing::bn256::Bn256,
    plonk::better_better_cs::{
        cs::Circuit, gates::main_gate_with_d_next::Width4MainGateWithDNext, proof::Proof,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct L1BatchProofForL1 {
    pub aggregation_result_coords: [[u8; 32]; 4],
    pub scheduler_proof: FinalProof,
}

pub type FinalProof = Proof<Bn256, GenericCircuit>;

#[derive(Clone)]
pub struct GenericCircuit {}

impl Circuit<Bn256> for GenericCircuit {
    type MainGate = Width4MainGateWithDNext;

    fn synthesize<CS: bellman::plonk::better_better_cs::cs::ConstraintSystem<Bn256>>(
        &self,
        _: &mut CS,
    ) -> Result<(), bellman::SynthesisError> {
        Ok(())
    }
}

/// Converts `U256` value into bytes array
pub fn u256_to_bytes_be(value: &U256) -> Vec<u8> {
    let mut bytes = vec![0u8; 32];
    value.to_big_endian(bytes.as_mut_slice());
    bytes
}
