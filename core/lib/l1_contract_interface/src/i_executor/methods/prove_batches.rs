use crypto_codegen::serialize_proof;
use zksync_prover_interface::outputs::L1BatchProofForL1;
use zksync_types::{
    commitment::L1BatchWithMetadata, ethabi::Token, web3::contract::tokens::Tokenizable, U256,
};

use crate::{i_executor::structures::StoredBatchInfo, Tokenize};

/// Input required to encode `proveBatches` call.
#[derive(Debug, Clone)]
pub struct ProveBatches {
    pub prev_l1_batch: L1BatchWithMetadata,
    pub l1_batches: Vec<L1BatchWithMetadata>,
    pub proofs: Proof,
    pub should_verify: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NewHorizenProof {
    pub attestation_id: u64,
    pub merkle_path: Vec<U256>,
    pub leaf_count: u32,
    pub index: u32,
}

#[derive(Debug, Clone)]
pub enum Proof {
    Proof(Vec<L1BatchProofForL1>),
    NewHorizenProof(NewHorizenProof),
}

impl Default for Proof {
    fn default() -> Self {
        Proof::Proof(Vec::new())
    }
}

impl From<Vec<L1BatchProofForL1>> for Proof {
    fn from(value: Vec<L1BatchProofForL1>) -> Self {
        Proof::Proof(value)
    }
}

impl From<NewHorizenProof> for Proof {
    fn from(value: NewHorizenProof) -> Self {
        Proof::NewHorizenProof(value)
    }
}

impl Tokenize for NewHorizenProof {
    fn into_tokens(self) -> Vec<Token> {
        vec![Token::Tuple(vec![
            Token::Uint(self.attestation_id.into()),
            Token::Array(self.merkle_path.into_iter().map(Token::Uint).collect()),
            Token::Uint(self.leaf_count.into()),
            Token::Uint(self.index.into()),
        ])]
    }
}

#[derive(Default)]
struct Zkproof(Vec<L1BatchProofForL1>);

impl Tokenize for Zkproof {
    fn into_tokens(self) -> Vec<Token> {
        let proofs = self.0;

        let proof = match proofs.first() {
            Some(L1BatchProofForL1 {
                scheduler_proof, ..
            }) => {
                let (_, proof) = serialize_proof(scheduler_proof);
                proof.into_iter().map(Token::Uint).collect()
            }
            None => Vec::new(),
        };

        vec![Token::Tuple(vec![
            Token::Array(Vec::new()),
            Token::Array(proof),
        ])]
    }
}

impl Tokenize for ProveBatches {
    fn into_tokens(self) -> Vec<Token> {
        let ProveBatches {
            prev_l1_batch,
            l1_batches,
            proofs,
            should_verify,
        } = self;
        let prev_l1_batch = StoredBatchInfo(&prev_l1_batch).into_token();
        let batches_arg = l1_batches
            .iter()
            .map(|batch| StoredBatchInfo(batch).into_token())
            .collect();
        let batches_arg = Token::Array(batches_arg);

        if should_verify {
            // currently we only support submitting a single proof
            assert_eq!(l1_batches.len(), 1);
            let proof_input = match proofs {
                Proof::Proof(p) => [
                    Zkproof(p).into_tokens(),
                    NewHorizenProof::default().into_tokens(),
                ]
                .concat(),
                Proof::NewHorizenProof(p) => {
                    [Zkproof::default().into_tokens(), p.into_tokens()].concat()
                }
            };
            assert_eq!(proof_input.len(), 2);

            [vec![prev_l1_batch, batches_arg], proof_input].concat()
        } else {
            [
                vec![prev_l1_batch, batches_arg],
                Zkproof::default().into_tokens(),
                NewHorizenProof::default().into_tokens(),
            ]
            .concat()
        }
    }
}
