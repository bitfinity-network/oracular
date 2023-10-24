use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::time::Duration;

use async_trait::async_trait;
use ic_exports::ic_cdk;
use ic_exports::ic_cdk::api::management_canister::http_request::{HttpResponse, TransformArgs};

use candid::{CandidType, Principal};
use did::{TransactionReceipt, H160, H256, U256};
use eth_signer::sign_strategy::SigningStrategy;
use eth_signer::sign_strategy::TransactionSigner;
use eth_signer::transaction::{SigningMethod, TransactionBuilder};
use evm_canister_client::EvmCanisterClient;
use ic_canister::{generate_idl, init, query, update, Canister, Idl, PreUpdate};
use ic_canister_client::IcCanisterClient;
use ic_exports::ic_kit::{self, ic};
use serde::{Deserialize, Serialize};

use crate::context::{get_base_context, Context, ContextImpl};
use crate::error::{Error, Result};
use crate::gen;
use crate::http::{self, transform};
use crate::processor::{EvmTransactionProcessorImpl, TxResultCallback};
use crate::state::{Pair, Settings, State};
use derive_more::From;
use ethers_core::abi::AbiEncode;

/// Type alias for the shared mutable context implementation we use in the canister
type SharedContext = Rc<RefCell<ContextImpl<EvmTransactionProcessorImpl>>>;

#[derive(Clone, Default)]
pub struct ContextWrapper(pub SharedContext);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, CandidType, Deserialize, From)]
pub struct ReserveAddressCallback {}

#[async_trait(?Send)]
impl TxResultCallback for ReserveAddressCallback {
    async fn processed(self, result: TransactionReceipt, context: &Rc<RefCell<dyn Context>>) {
        let evm_client = {
            let ctx = context.borrow();
            ctx.get_evm_client()
        };

        if let Ok(Ok(_)) = evm_client
            .reserve_address(ic_kit::ic::id(), result.transaction_hash)
            .await
        {
            ic::print("Reserved address successfully");
        }

        ic::print("Address not reserved")
    }

    async fn skipped(self, _context: &Rc<RefCell<dyn Context>>) {}
}

#[derive(Canister, Clone)]
pub struct Oracular {
    #[id]
    pub id: Principal,
    pub context: ContextWrapper,
}

impl PreUpdate for Oracular {}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct InitData {
    pub owner: Principal,
    pub evm: Principal,
    pub signing_strategy: SigningStrategy,
    evm_chain_id: u64,
}

impl Oracular {
    fn with_state<R>(&self, f: impl FnOnce(&State) -> R) -> R {
        let ctx = self.context.0.borrow();
        let res = f(&ctx.get_state());
        res
    }

    fn with_state_mut<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        let ctx = self.context.0.borrow();
        let res = f(&mut ctx.mut_state());
        res
    }
    /// Process the callbacks.
    /// This method is supposed to be used in tests only.
    pub async fn process_callbacks(&mut self) {
        let context = self.context.0.clone();
        Self::process_callbacks_impl(&context).await;
    }

    async fn process_callbacks_impl(context: &SharedContext) {
        let base_context = get_base_context(context);
        let tx_processor = context.borrow().get_tx_processor_impl();

        tx_processor.process_transactions(&base_context).await;
    }

    fn update_price_feed_timer(&self) {
        let context = self.context.0.clone();
        ic_exports::ic_cdk_timers::set_timer_interval(Duration::from_secs(60 * 60), move || {
            let pairs = context
                .borrow()
                .get_state()
                .pair_storage()
                .all_pairs_with_address();

            // For each pair, update the price by sending a transaction to the aggregator contract
        });
    }

    fn sync_pair_prices(&self) {
        let context = self.context.0.clone();
        ic_exports::ic_cdk_timers::set_timer_interval(Duration::from_secs(300), move || {
            let context = context.clone();
            ic_cdk::spawn(async move {
                let res = http::update_pair_price(&get_base_context(&context)).await;

                ic::print(format!("res: {:?}", res));
            })
        });
    }

    #[init]
    pub fn init(&mut self, data: InitData) {
        let settings = Settings::new(
            data.owner,
            data.evm,
            data.signing_strategy,
            data.evm_chain_id,
        );

        check_anonymous_principal(data.owner).expect("invalid owner");

        self.with_state_mut(|state| state.reset(settings));

        // #[cfg(target_arch = "wasm32")]
        {
            use std::time::Duration;

            use ic_exports::ic_cdk_timers::set_timer_interval;

            let context = self.context.0.clone();
            set_timer_interval(Duration::from_secs(60), move || {
                let context = context.clone();
                ic::spawn(async move {
                    Self::process_callbacks_impl(&context).await;
                });
            });

            self.sync_pair_prices();
            self.update_price_feed_timer();
        }
    }

    #[update]
    pub async fn get_oracle_address(&self) -> Result<H160> {
        let signer = self.with_state(|state| state.signer.get_transaction_signer());

        signer
            .get_address()
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }

    #[update]
    pub async fn reserve_oracle_agent(&self) -> Result<()> {
        let client = self.context.0.borrow().get_evm_client();

        let signer = self.with_state(|state| state.signer.get_transaction_signer());

        let address = signer.get_address().await?;

        let chain_id = client.eth_chain_id().await?;

        let transaction = TransactionBuilder {
            from: &address.clone(),
            to: Some(address),
            nonce: U256::zero(),
            value: U256::zero(),
            gas: 23_000u64.into(),
            gas_price: None,
            input: ic_kit::ic::id().as_slice().to_vec(),
            signature: SigningMethod::None,
            chain_id,
        }
        .calculate_hash_and_build()?;

        let tx_hash = client.send_raw_transaction(transaction).await??;

        let callback = ReserveAddressCallback {};

        self.context
            .0
            .borrow()
            .get_tx_processor()
            .register_transaction(tx_hash.clone(), callback.into());

        Ok(())
    }

    #[update]
    pub async fn create_price_pair(
        &mut self,
        pair: Pair,
        decimal: U256,
        description: String,
        version: U256,
    ) -> Result<()> {
        // Check pair is in the list of pairs
        let pair_storage = self.with_state(|state| state.pair_storage.clone());
        if !pair_storage.check_pair_exists(&pair.id()) {
            return Err(Error::Internal(format!(
                "Pair {} not in the list of pairs",
                pair
            )));
        }

        // check if the pair exists
        http::check_pair_exist(&pair).await?;

        let contract_service = {
            let ctx = self.context.0.borrow();
            ctx.get_contract_service()
        };

        contract_service
            .create_pair_feed(
                &get_base_context(&self.context.0),
                pair,
                decimal,
                description,
                version,
            )
            .await?;

        Ok(())
    }

    #[query]
    pub fn owner(&self) -> Principal {
        self.with_state(|state| state.owner())
    }

    #[query]
    pub fn evm(&self) -> Principal {
        self.with_state(|state| state.evm())
    }

    #[update]
    pub fn set_owner(&mut self, owner: Principal) {
        // TODO: check if the owner is a valid principal
        self.with_state_mut(|state| state.set_owner(owner));
    }

    #[update]
    pub fn set_evm(&mut self, evm: Principal) {
        self.with_state_mut(|state| state.set_evm(evm));
    }

    /// Requirements for Http outcalls, used to ignore small differences in the data obtained
    /// by different nodes of the IC subnet to reach a consensus, more info:
    /// https://internetcomputer.org/docs/current/developer-docs/integrations/http_requests/http_requests-how-it-works#transformation-function
    #[query]
    fn transform(&self, raw: TransformArgs) -> HttpResponse {
        transform(raw)
    }

    /// Returns candid IDL.
    /// This should be the last fn to see previous endpoints in macro.
    pub fn idl() -> Idl {
        generate_idl!()
    }
}

/// inspect function to check whether the provided principal is anonymous
fn check_anonymous_principal(principal: Principal) -> anyhow::Result<()> {
    if principal == Principal::anonymous() {
        anyhow::bail!("Principal cannot be anonymous.");
    }

    Ok(())
}
