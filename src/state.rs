mod oracle_storage;
mod settings;
mod signer;

use candid::Principal;

use self::oracle_storage::OracleStorage;
pub use self::settings::Settings;
use self::signer::SignerInfo;

pub use oracle_storage::UpdateOracleMetadata;

#[derive(Debug, Default, Clone)]
pub struct State {
    /// Transaction signing info.
    pub signer: SignerInfo,
    /// Pair storage.
    pub oracle_storage: OracleStorage,
}

impl State {
    /// Clear state.
    pub fn reset(&mut self, settings: Settings) {
        Settings::update(|s| *s = settings.clone());

        self.signer.clear();

        self.oracle_storage.clear();
    }

    pub fn owner(&self) -> Principal {
        Settings::read(|s| s.owner)
    }

    pub fn set_owner(&mut self, owner: Principal) {
        Settings::update(|s| s.owner = owner);
    }

    pub fn mut_oracle_storage(&mut self) -> &mut OracleStorage {
        &mut self.oracle_storage
    }

    pub fn oracle_storage(&self) -> &OracleStorage {
        &self.oracle_storage
    }

    pub fn ic_eth(&self) -> Principal {
        Settings::read(|s| s.ic_eth)
    }

    pub fn set_ic_eth(&mut self, ic_eth: Principal) {
        Settings::update(|s| s.ic_eth = ic_eth);
    }

    pub fn signer(&self) -> &SignerInfo {
        &self.signer
    }
}
