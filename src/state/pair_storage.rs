use std::borrow::Cow;
use std::cell::RefCell;

use candid::CandidType;
use did::codec::{decode, encode};
use did::H160;
use ic_stable_structures::{
    get_memory_by_id, BTreeMapStructure, Bound, MultimapStructure, StableBTreeMap, StableMultimap,
    Storable,
};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::memory::{
    self, MemoryType, MEMORY_MANAGER, PAIR_MAP_ADDRESS_MEMORY_ID, PAIR_STORAGE_MEMORY_ID,
};
use std::fmt::Display;

use serde::Serialize;

#[derive(Debug, CandidType, Clone, Deserialize, Serialize)]
pub struct Pair {
    base_currency: String,
    quote_currency: String,
}

impl Display for Pair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.base_currency, self.quote_currency)
    }
}

impl Pair {
    /// Create a new pair.
    pub fn id(&self) -> String {
        format!(
            "{}-{}",
            self.base_currency.to_lowercase(),
            self.quote_currency.to_lowercase()
        )
    }
}

impl Storable for Pair {
    fn to_bytes(&self) -> Cow<[u8]> {
        encode(self).into()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        decode(&bytes)
    }

    const BOUND: ic_stable_structures::Bound = ic_stable_structures::Bound::Unbounded;
}

#[derive(Debug, CandidType, Deserialize)]
pub struct PairData {
    pair: Pair,
    price: u64,
    timestamp: u64,
}

impl Storable for PairData {
    fn to_bytes(&self) -> Cow<[u8]> {
        encode(self).into()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        decode(&bytes)
    }

    const BOUND: Bound = Bound::Unbounded;
}

#[derive(Debug, Default, Clone)]
pub struct PairStorage {}

impl PairStorage {
    pub fn get(&self, id: &Id) -> Result<Pair> {
        PAIR_STORAGE.with(|storage| {
            let storage = storage.borrow();
            let pair_data = storage.get(id).ok_or(Error::PairNotFound)?;
            Ok(pair_data.pair.clone())
        })
    }

    pub fn add_address(&self, id: &Id, address: H160) {
        PAIR_MAP_ADDRESS.with(|storage| {
            let mut storage = storage.borrow_mut();
            storage.insert(id.clone(), address);
        })
    }
    pub fn check_pair_exists(&self, id: &Id) -> bool {
        PAIR_STORAGE.with(|storage| {
            let storage = storage.borrow();
            storage.get(id).is_some()
        })
    }

    pub fn add_pair(&self, id: &Id, pair: Pair, timestamp: u64, price: u64) -> Result<()> {
        PAIR_STORAGE.with(|storage| {
            let mut storage = storage.borrow_mut();
            // Check if pair already exists
            if self.check_pair_exists(id) {
                return Err(Error::PairAlreadyExists);
            }

            storage.insert(
                id.clone(),
                PairData {
                    pair,
                    timestamp,
                    price,
                },
            );

            Ok(())
        })
    }

    pub fn remove_pair(&self, id: &Id) -> Result<()> {
        // Check if exists
        if !self.check_pair_exists(id) {
            return Err(Error::PairNotFound);
        }
        PAIR_STORAGE.with(|storage| {
            let mut storage = storage.borrow_mut();
            storage.remove(id);
        });

        PAIR_MAP_ADDRESS.with(|storage| {
            let mut storage = storage.borrow_mut();
            storage.remove(id);
        });

        Ok(())
    }

    pub fn all_pairs(&self) -> Vec<Pair> {
        PAIR_STORAGE.with(|storage| {
            let storage = storage.borrow();
            storage.iter().map(|(_, v)| v.pair.clone()).collect()
        })
    }

    pub fn all_pairs_with_address(&self) -> Vec<(Pair, H160)> {
        PAIR_MAP_ADDRESS.with(|storage| {
            let storage = storage.borrow();
            storage
                .iter()
                .map(|(k, v)| (self.get(&k).expect("pair should be present"), v.clone()))
                .collect()
        })
    }

    pub fn update_pair(&self, id: &Id, price: u64, timestamp: u64) -> Result<()> {
        PAIR_STORAGE.with(|storage| {
            let mut storage = storage.borrow_mut();

            let pair_data = storage.get(id).ok_or(Error::PairNotFound)?;

            storage.insert(
                id.clone(),
                PairData {
                    pair: pair_data.pair.clone(),
                    timestamp,
                    price,
                },
            );

            Ok(())
        })
    }

    pub fn clear(&self) {
        PAIR_STORAGE.with(|storage| {
            let mut storage = storage.borrow_mut();
            storage.clear();
        });

        PAIR_MAP_ADDRESS.with(|storage| {
            let mut storage = storage.borrow_mut();
            storage.clear();
        });
    }
}

thread_local! {
    static PAIR_STORAGE: RefCell<StableBTreeMap<Id, PairData, MemoryType>> = RefCell::new(StableBTreeMap::new(get_memory_by_id(&MEMORY_MANAGER, PAIR_STORAGE_MEMORY_ID)));

    static PAIR_MAP_ADDRESS: RefCell<StableBTreeMap<Id,  H160, MemoryType>> = RefCell::new(StableBTreeMap::new(get_memory_by_id(&MEMORY_MANAGER, PAIR_MAP_ADDRESS_MEMORY_ID)));
}

pub type Id = String;
