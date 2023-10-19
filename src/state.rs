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

pub use self::settings::Settings;
use self::signer::SignerInfo;

#[derive(Debug, Default, Clone)]
pub struct State {
    /// Transaction signing info.
    pub signer: SignerInfo,
}

impl State {
    ///  `settings`.
    pub fn new(settings: Settings) -> Self {
        let mut new = Self::default();
        // clear all data in stable storage
        new.clear(Some(settings));
        new
    }

    /// Clear state.

    pub fn clear(&mut self, settings: Option<Settings>) {
        Settings::update(|s| *s = settings.clone().unwrap_or_default());
    }
}
