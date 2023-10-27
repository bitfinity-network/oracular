pub mod state_machine;

use candid::utils::ArgumentEncoder;
use candid::Principal;

use ic_canister_client::CanisterClient;

use crate::utils::error::Result;
use oracular::canister::InitData;

use crate::utils::wasm::{get_eth_rpc_bytecode, get_oracular_canister_bytecode};

#[async_trait::async_trait]
pub trait TestContext {
    type Client: CanisterClient + Send + Sync;

    /// Returns principals for canisters in the context.
    fn canisters(&self) -> TestCanisters;

    /// Returns client for the canister.
    fn client(&self, canister: Principal, caller: &str) -> Self::Client;

    /// Principal to use for canisters initialization.
    fn admin(&self) -> Principal;

    /// Principal to use for canisters initialization.
    fn admin_name(&self) -> &str;

    /// Reinstalls the canister.
    async fn reinstall_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> Result<()>;

    /// Upgrades the canister.
    async fn upgrade_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> Result<()>;

    async fn reinstall_eth_rpc_canister(&self) -> Result<()> {
        eprintln!("reinstalling ETH RPC canister");
        let wasm = get_eth_rpc_bytecode().await;

        self.reinstall_canister(self.canisters().ic_eth_rpc, wasm, ((),))
            .await?;

        Ok(())
    }

    async fn reinstall_oracular_canister(&self) -> Result<()> {
        eprintln!("reinstalling Oracular canister");
        let init_data = oracular_init_data(self.admin(), self.canisters().ic_eth_rpc);

        let wasm = get_oracular_canister_bytecode().await;
        self.reinstall_canister(self.canisters().oracular, wasm, (init_data,))
            .await?;

        Ok(())
    }
}

pub fn oracular_init_data(owner: Principal, ic_eth: Principal) -> InitData {
    InitData {
        owner,
        ic_eth_rpc: Some(ic_eth),
    }
}

#[derive(Debug, Clone)]
pub struct TestCanisters {
    pub oracular: Principal,
    pub ic_eth_rpc: Principal,
}
