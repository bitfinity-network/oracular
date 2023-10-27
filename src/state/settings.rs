use std::borrow::Cow;
use std::cell::RefCell;

use candid::{CandidType, Principal};
use did::codec;
use ic_stable_structures::{Bound, CellStructure, StableCell, Storable};
use serde::{Deserialize, Serialize};

use crate::memory::{MemoryType, MEMORY_MANAGER, SETTINGS_MEMORY_ID};

#[derive(Debug, Clone, Serialize, Deserialize, CandidType)]
pub struct Settings {
    pub owner: Principal,
    pub ic_eth: Principal,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            owner: Principal::management_canister(),
            ic_eth: Principal::management_canister(),
        }
    }
}

impl Settings {
    pub fn new(owner: Principal, ic_eth: Principal) -> Self {
        Self { owner, ic_eth }
    }

    pub fn read<F, T>(f: F) -> T
    where
        for<'a> F: FnOnce(&'a Self) -> T,
    {
        SETTINGS_CELL.with(|cell| f(cell.borrow().get()))
    }

    pub fn update<F, T>(f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        SETTINGS_CELL.with(|cell| {
            let mut new_settings = cell.borrow().get().clone();
            let result = f(&mut new_settings);
            cell.borrow_mut()
                .set(new_settings)
                .expect("failed to set evm settings");
            result
        })
    }
}

impl Storable for Settings {
    fn to_bytes(&self) -> Cow<[u8]> {
        codec::encode(&self).into()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        codec::decode(&bytes)
    }

    const BOUND: ic_stable_structures::Bound = Bound::Bounded {
        max_size: 55,
        is_fixed_size: true,
    };
}

thread_local! {
    static SETTINGS_CELL: RefCell<StableCell<Settings, MemoryType>> = {
        RefCell::new(StableCell::new(MEMORY_MANAGER.with(|mm| mm.get(SETTINGS_MEMORY_ID)), Settings::default()).expect("failed to initialize settings"))
    };
}
