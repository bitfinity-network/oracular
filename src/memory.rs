use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{MemoryId, MemoryManager, VirtualMemory};

thread_local! {
    pub static MEMORY_MANAGER: MemoryManager<DefaultMemoryImpl> = MemoryManager::init(DefaultMemoryImpl::default());
}

pub type MemoryType = VirtualMemory<DefaultMemoryImpl>;

pub const SETTINGS_MEMORY_ID: MemoryId = MemoryId::new(1);
pub const ORACLE_STORAGE_MEMORY_ID: MemoryId = MemoryId::new(2);
pub const TX_SIGNER_MEMORY_ID: MemoryId = MemoryId::new(3);
