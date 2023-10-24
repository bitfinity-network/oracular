mod pair_storage;
mod settings;
mod signer;

use std::borrow::Cow;
use std::cell::RefCell;

use candid::{CandidType, Principal};
use did::codec;
use ic_stable_structures::{
    get_memory_by_id, CellStructure, MemoryId, StableCell, Storable, VirtualMemory,
};
use serde::{Deserialize, Serialize};

use crate::memory::{MemoryType, MEMORY_MANAGER, SETTINGS_MEMORY_ID};

use self::pair_storage::PairStorage;
pub use self::settings::Settings;
use self::signer::SignerInfo;

pub use self::pair_storage::Pair;

#[derive(Debug, Default, Clone)]
pub struct State {
    /// Transaction signing info.
    pub signer: SignerInfo,
    /// Pair storage.
    pub pair_storage: PairStorage,
}

impl State {
    /// Clear state.
    pub fn reset(&mut self, settings: Settings) {
        Settings::update(|s| *s = settings.clone());

        self.signer
            .reset(settings.signing_strategy, settings.evm_chain_id as u32)
            .expect("failed to set signer");

        self.pair_storage.clear();
    }

    pub fn owner(&self) -> Principal {
        Settings::read(|s| s.owner)
    }

    pub fn evm(&self) -> Principal {
        Settings::read(|s| s.evm)
    }

    pub fn set_owner(&mut self, owner: Principal) {
        Settings::update(|s| s.owner = owner);
    }

    pub fn set_evm(&mut self, evm: Principal) {
        Settings::update(|s| s.evm = evm);
    }

    pub fn chain_id(&self) -> u64 {
        Settings::read(|s| s.evm_chain_id)
    }

    pub fn set_chain_id(&mut self, chain_id: u64) {
        Settings::update(|s| s.evm_chain_id = chain_id);
    }

    pub fn mut_pair_storage(&mut self) -> &mut PairStorage {
        &mut self.pair_storage
    }

    pub fn pair_storage(&self) -> &PairStorage {
        &self.pair_storage
    }
}
