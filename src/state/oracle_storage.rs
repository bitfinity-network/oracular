use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::BTreeMap;

use candid::{CandidType, Principal};

use did::ic::StorablePrincipal;
use did::H160;
use ic_exports::ic_cdk_timers::TimerId;
use ic_stable_structures::{
    Bound, ChunkSize, SlicedStorable, StableUnboundedMap, Storable, UnboundedMapStructure,
};
use serde::Deserialize;

use crate::canister::{EvmDestination, Origin};
use crate::error::{Error, Result};
use crate::memory::{MemoryType, MEMORY_MANAGER, ORACLE_STORAGE_MEMORY_ID};

use serde::Serialize;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleMetadata {
    pub origin: Origin,
    pub timer_interval: u64,
    pub timer_id: TimerId,
    pub evm: EvmDestination,
}

impl Storable for VecMetadata {
    fn to_bytes(&self) -> Cow<[u8]> {
        did::codec::bincode_encode(&self).into()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        did::codec::bincode_decode(&bytes)
    }

    const BOUND: Bound = Bound::Unbounded;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VecMetadata(BTreeMap<H160, OracleMetadata>);

impl SlicedStorable for VecMetadata {
    const CHUNK_SIZE: ChunkSize = 64;
}

/// The pair storage. Stores the pair data.
#[derive(Debug, Default, Clone)]
pub struct OracleStorage {}

impl OracleStorage {
    #[allow(clippy::too_many_arguments)]
    pub fn add_oracle(
        &self,
        principal: Principal,
        origin: Origin,
        timestamp: u64,
        timer_id: TimerId,
        evm: EvmDestination,
    ) {
        ORACLE_STORAGE.with(|storage| {
            let storage = storage.borrow_mut();

            let mut vec = storage
                .get(&StorablePrincipal(principal))
                .unwrap_or_default();

            vec.0.insert(
                evm.contract.clone(),
                OracleMetadata {
                    origin,
                    timer_id,
                    timer_interval: timestamp,
                    evm,
                },
            );
        });
    }

    pub fn get_oracle_by_address(
        &self,
        principal: Principal,
        evm_contract_address: H160,
    ) -> Result<OracleMetadata> {
        ORACLE_STORAGE.with(|storage| {
            let storage = storage.borrow();

            let vec = storage
                .get(&StorablePrincipal(principal))
                .ok_or(Error::OracleNotFound)?;

            vec.0
                .get(&evm_contract_address)
                .cloned()
                .ok_or(Error::OracleNotFound)
        })
    }

    pub fn get_oracles(&self) -> Vec<(Principal, VecMetadata)> {
        ORACLE_STORAGE.with(|storage| {
            let storage = storage.borrow();
            storage.iter().map(|(k, v)| (k.0, v.clone())).collect()
        })
    }

    pub fn remove_oracle_by_address(
        &self,
        principal: Principal,
        evm_contract_address: H160,
    ) -> Result<()> {
        ORACLE_STORAGE.with(|storage| {
            let mut storage = storage.borrow_mut();
            let mut map = storage
                .get(&StorablePrincipal(principal))
                .ok_or(Error::OracleNotFound)?;

            map.0.remove(&evm_contract_address);

            if map.0.is_empty() {
                storage.remove(&StorablePrincipal(principal));
            }

            Ok(())
        })
    }

    pub fn update_oracle_metadata(
        &mut self,
        principal: Principal,
        evm_contract_address: H160,
        timer_id: Option<TimerId>,
        update_metadata: UpdateOracleMetadata,
    ) -> Result<()> {
        ORACLE_STORAGE.with(|storage| {
            let storage = storage.borrow_mut();

            let mut vec_metadata = storage
                .get(&StorablePrincipal(principal))
                .ok_or(Error::OracleNotFound)?;

            let metadata = vec_metadata
                .0
                .get_mut(&evm_contract_address)
                .ok_or(Error::OracleNotFound)?;

            Self::update_field(&mut metadata.origin, update_metadata.origin);
            Self::update_field(&mut metadata.timer_interval, update_metadata.timestamp);
            Self::update_field(&mut metadata.evm, update_metadata.evm);
            Self::update_field(&mut metadata.timer_id, timer_id);

            Ok(())
        })
    }

    pub fn clear(&self) {
        ORACLE_STORAGE.with(|storage| {
            let mut storage = storage.borrow_mut();
            storage.clear();
        });
    }

    fn update_field<T: Clone>(target: &mut T, update: Option<T>) {
        if let Some(new_val) = update {
            *target = new_val;
        }
    }
}

thread_local! {
    static ORACLE_STORAGE: RefCell<StableUnboundedMap<StorablePrincipal, VecMetadata, MemoryType>> = RefCell::new(StableUnboundedMap::new(MEMORY_MANAGER.with(|mm|mm.get(ORACLE_STORAGE_MEMORY_ID))));
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, CandidType)]

pub struct UpdateOracleMetadata {
    pub origin: Option<Origin>,
    pub method: Option<String>,
    pub json_path: Option<String>,
    pub evm: Option<EvmDestination>,
    pub timestamp: Option<u64>,
}

impl UpdateOracleMetadata {
    pub fn is_none(&self) -> bool {
        self.origin.is_none()
            && self.method.is_none()
            && self.json_path.is_none()
            && self.evm.is_none()
            && self.timestamp.is_none()
    }
}
