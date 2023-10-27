use core::fmt;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use candid::utils::ArgumentEncoder;
use candid::{Encode, Principal};
use ic_canister_client::StateMachineCanisterClient;
use ic_exports::ic_kit::mock_principals::{alice, bob, john};
use ic_exports::ic_test_state_machine::StateMachine;

use super::{TestCanisters, TestContext};
use crate::context::oracular_init_data;
use crate::utils::error::Result;
use crate::utils::get_state_machine_bin_path;
use crate::utils::wasm::{get_eth_rpc_bytecode, get_oracular_canister_bytecode};
use once_cell::sync::Lazy;
use tokio::sync::{Mutex, MutexGuard, OnceCell};
pub struct StateMachineTestContext {
    pub env: Arc<Mutex<StateMachine>>,
    pub canisters: TestCanisters,
}

impl StateMachineTestContext {
    pub async fn new() -> Self {
        let env = StateMachine::new(&get_state_machine_bin_path().await, false);
        let canisters = StateMachineTestContext::deploy_canisters(&env, Self::admin()).await;
        let env = Arc::new(Mutex::new(env));
        StateMachineTestContext { env, canisters }
    }

    pub fn admin() -> Principal {
        bob()
    }

    pub async fn reset_and_lock() -> impl Deref<Target = Self> {
        static CONTEXT: Lazy<Mutex<OnceCell<StateMachineTestContext>>> =
            Lazy::new(Default::default);
        let ctx = CONTEXT.lock().await;
        ctx.get_or_init(Self::new).await;
        let ctx = MutexGuard::map(ctx, |cell| cell.get_mut().unwrap());

        ctx.reinstall_oracular_canister().await.unwrap();
        ctx.reinstall_eth_rpc_canister().await.unwrap();

        ctx
    }

    async fn deploy_canisters(env: &StateMachine, admin: Principal) -> TestCanisters {
        let ic_eth_rpc_canister = deploy_ic_eth_rpc_canister(env, admin).await.unwrap();
        let oracle_canister = deploy_oracular_canister(env, admin, ic_eth_rpc_canister)
            .await
            .unwrap();

        TestCanisters {
            oracular: oracle_canister,
            ic_eth_rpc: ic_eth_rpc_canister,
        }
    }

    pub async fn advance_time(&self, time: Duration) {
        let client = self.client(self.canisters.oracular, ADMIN);
        client
            .with_state_machine(move |env, _, _| {
                env.advance_time(time);
                env.tick();
            })
            .await
    }
}

#[async_trait::async_trait]
impl TestContext for StateMachineTestContext {
    type Client = StateMachineCanisterClient;

    fn canisters(&self) -> TestCanisters {
        self.canisters.clone()
    }

    fn client(&self, canister: Principal, caller: &str) -> Self::Client {
        let caller_principal = match caller {
            ADMIN => Self::admin(),
            JOHN => john(),
            ALICE => alice(),
            _ => panic!("unexpected caller"),
        };
        StateMachineCanisterClient::new(self.env.clone(), canister, caller_principal)
    }

    fn admin(&self) -> Principal {
        Self::admin()
    }

    fn admin_name(&self) -> &str {
        ADMIN
    }

    async fn reinstall_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> Result<()> {
        let client = StateMachineCanisterClient::new(self.env.clone(), canister, self.admin());
        let args = candid::encode_args(args).unwrap();
        client
            .with_state_machine(move |env, canister, caller| {
                env.reinstall_canister(canister, wasm, args, Some(caller))
            })
            .await?;
        Ok(())
    }

    async fn upgrade_canister(
        &self,
        canister: Principal,
        wasm: Vec<u8>,
        args: impl ArgumentEncoder + Send,
    ) -> Result<()> {
        let args = candid::encode_args(args).unwrap();
        let client = StateMachineCanisterClient::new(self.env.clone(), canister, self.admin());
        client
            .with_state_machine(move |env, canister, caller| {
                env.upgrade_canister(canister, wasm, args, Some(caller))
            })
            .await?;

        Ok(())
    }
}

const ADMIN: &str = "admin";
const JOHN: &str = "john";
const ALICE: &str = "alice";

impl fmt::Debug for StateMachineTestContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StateMachineTestContext")
            .field("env", &"State machine tests client")
            .field("canisters", &self.canisters)
            .finish()
    }
}

async fn deploy_oracular_canister(
    env: &StateMachine,
    admin: Principal,
    ic_eth: Principal,
) -> Result<Principal> {
    let wasm = get_oracular_canister_bytecode().await;
    println!("Creating Oracular canister");
    let init_data = oracular_init_data(admin, ic_eth);
    let payload = Encode!(&init_data)?;
    let oracular_canister = env.create_canister(Some(admin));
    env.add_cycles(oracular_canister, u128::MAX);
    env.install_canister(oracular_canister, wasm, payload, Some(admin));
    println!("Oracular Canister created {oracular_canister}");
    Ok(oracular_canister)
}

async fn deploy_ic_eth_rpc_canister(env: &StateMachine, admin: Principal) -> Result<Principal> {
    let wasm = get_eth_rpc_bytecode().await;
    println!("Creating ETH RPC canister");
    let ic_eth_rpc_canister = env.create_canister(Some(admin));
    env.add_cycles(ic_eth_rpc_canister, u128::MAX);
    env.install_canister(
        ic_eth_rpc_canister,
        wasm,
        Encode!(&()).unwrap(),
        Some(admin),
    );
    println!("ETH RPC Canister created {ic_eth_rpc_canister}");
    Ok(ic_eth_rpc_canister)
}
