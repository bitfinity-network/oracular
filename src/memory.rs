use ic_stable_structures::{
    DefaultMemoryManager, DefaultMemoryResourceType, DefaultMemoryType, MemoryId,
};

pub type MemoryType = DefaultMemoryType;

thread_local! {
    pub static MEMORY_MANAGER: DefaultMemoryManager = DefaultMemoryManager::init(DefaultMemoryResourceType::default());
}

pub const SETTINGS_MEMORY_ID: MemoryId = MemoryId::new(1);
