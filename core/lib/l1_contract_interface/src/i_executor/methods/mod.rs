//! Utilities for encoding input data for methods defined in `IExecutor.sol`.

pub use self::{
    commit_batches::{CommitBatchesRollup, CommitBatchesValidium},
    execute_batches::ExecuteBatches,
    prove_batches::{NewHorizenProof, ProveBatches},
};

mod commit_batches;
mod execute_batches;
mod prove_batches;
