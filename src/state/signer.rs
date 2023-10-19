use std::cell::RefCell;

use eth_signer::ic_sign::SigningKeyId;
use eth_signer::sign_strategy::{
    ManagementCanisterSigner, SigningStrategy, TransactionSigner, TxSigner,
};
use ic_stable_structures::{get_memory_by_id, CellStructure, MemoryId, StableCell};

use crate::memory::{MemoryType, MEMORY_MANAGER};

/// A component that provides the access to the signer
#[derive(Debug, Default, Clone)]
pub struct SignerInfo {}

impl SignerInfo {
    /// Reset the signer with the given strategy and chain id.
    pub fn reset(&self, signing_type: SigningStrategy, chain_id: u32) -> anyhow::Result<()> {
        let signer = signing_type
            .make_signer(chain_id as _)
            .map_err(|e| anyhow::anyhow!("failed to create transaction signer: {}", e))?;

        TX_SIGNER.with(|s| {
            s.borrow_mut()
                .set(signer)
                .expect("failed to update transaction signer")
        });

        Ok(())
    }

    /// Returns transaction signer
    pub fn get_transaction_signer(&self) -> impl TransactionSigner {
        TX_SIGNER.with(|s| s.borrow().get().clone())
    }
}

thread_local! {
    static TX_SIGNER: RefCell<StableCell<TxSigner, MemoryType>> = RefCell::new(StableCell::new(get_memory_by_id(&MEMORY_MANAGER,TX_SIGNER_MEMORY_ID), TxSigner::ManagementCanister(ManagementCanisterSigner::new(SigningKeyId::Test, vec![]))).expect("failed to initialize transaction signer"))
}

pub const TX_SIGNER_MEMORY_ID: MemoryId = MemoryId::new(2);
