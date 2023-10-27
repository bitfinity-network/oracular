use std::borrow::Cow;
use std::cell::RefCell;

use candid::{CandidType, Principal};
use did::error::EvmError;
use did::ic::StorablePrincipal;
use did::transaction::Signature;
use did::H160;
use eth_signer::ic_sign::{DerivationPath, SigningKeyId};
use eth_signer::sign_strategy::{IcSigner, TransactionSigner};
use ethers_core::types::transaction::eip2718::TypedTransaction;
use ic_stable_structures::{BTreeMapStructure, Bound, StableBTreeMap, Storable};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::memory::{MemoryType, MEMORY_MANAGER, TX_SIGNER_MEMORY_ID};
/// A component that provides the access to the signer
#[derive(Debug, Default, Clone)]
pub struct SignerInfo;

impl SignerInfo {
    pub fn get_oracle_signer(&self, principal: Principal) -> Result<impl TransactionSigner> {
        TX_SIGNER.with(|tx_signer| {
            let tx_signer = tx_signer.borrow();
            let signer = tx_signer
                .get(&StorablePrincipal(principal))
                .ok_or(Error::from(format!(
                    "signer for principal {} not found",
                    principal
                )))?;
            let oracle_signer = OracleSigner::new(signer);

            Ok(oracle_signer)
        })
    }

    pub fn get_signer_address(&self, principal: Principal) -> Result<H160> {
        TX_SIGNER.with(|tx_signer| {
            let tx_signer = tx_signer.borrow_mut();
            let signer = tx_signer
                .get(&StorablePrincipal(principal))
                .ok_or(Error::from(format!(
                    "signer for principal {} not found",
                    principal
                )))?;
            Ok(signer)
        })
    }

    pub fn add_signer(&self, principal: Principal, address: H160) {
        TX_SIGNER.with(|tx_signer| {
            let mut tx_signer = tx_signer.borrow_mut();
            tx_signer
                .insert(StorablePrincipal(principal), address)
                .expect("failed to add signer");
        })
    }

    pub fn clear(&self) {
        TX_SIGNER.with(|tx_signer| {
            let mut tx_signer = tx_signer.borrow_mut();
            tx_signer.clear();
        })
    }
}

#[derive(CandidType, Clone, Deserialize, Debug)]
pub struct OracleSigner {
    pub(super) key_id: SigningKeyId,
    pub(super) derivation_path: DerivationPath,
}

impl OracleSigner {
    fn new(address: H160) -> Self {
        let address_to_bytes = address.0.as_bytes().to_vec();
        Self {
            key_id: SigningKeyId::Test,
            derivation_path: vec![address_to_bytes],
        }
    }
}

#[async_trait::async_trait(?Send)]
impl TransactionSigner for OracleSigner {
    /// Returns the `sender` address for the given identity
    async fn get_address(&self) -> did::error::Result<H160> {
        let pubkey = IcSigner {}
            .public_key(self.key_id, self.derivation_path.clone())
            .await
            .map_err(|e| EvmError::from(format!("failed to get address: {e}")))?;
        let address: H160 = IcSigner
            .pubkey_to_address(&pubkey)
            .map_err(|e| {
                EvmError::Internal(format!("failed to convert public key to address: {e}"))
            })?
            .into();

        Ok(address)
    }

    /// Sign the created transaction
    async fn sign_transaction(
        &self,
        transaction: &TypedTransaction,
    ) -> did::error::Result<Signature> {
        IcSigner {}
            .sign_transaction(transaction, self.key_id, self.derivation_path.clone())
            .await
            .map_err(|e| EvmError::from(format!("failed to get message signature: {e}")))
            .map(Into::into)
    }

    /// Sign the given digest
    async fn sign_digest(&self, digest: [u8; 32]) -> did::error::Result<Signature> {
        let address = self.get_address().await?;
        IcSigner
            .sign_digest(
                &address.into(),
                digest,
                self.key_id,
                self.derivation_path.clone(),
            )
            .await
            .map_err(|e| EvmError::from(format!("failed to get message signature: {e}")))
            .map(Into::into)
    }
}

impl Storable for OracleSigner {
    fn to_bytes(&self) -> Cow<[u8]> {
        did::codec::encode(&self).into()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        did::codec::decode(&bytes)
    }

    const BOUND: ic_stable_structures::Bound = Bound::Unbounded;
}

thread_local! {
    static TX_SIGNER: RefCell<StableBTreeMap<StorablePrincipal, H160, MemoryType>> = RefCell::new(StableBTreeMap::new(MEMORY_MANAGER.with(|mm|mm.get(TX_SIGNER_MEMORY_ID))))

}
