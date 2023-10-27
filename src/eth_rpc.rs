use std::cell::RefCell;
use std::rc::Rc;

use candid::{CandidType, Principal};
use ic_canister_client::CanisterClient;
use serde::{Deserialize, Serialize};

use crate::context::Context;

use crate::error::Result;

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum Source {
    Url(String),
    Provider(u64),
    Chain(u64),
    Service {
        hostname: String,
        chain_id: Option<u64>,
    },
}

#[derive(Debug, CandidType, Deserialize)]
pub struct ProviderView {
    pub provider_id: u64,
    pub owner: Principal,
    pub chain_id: u64,
    pub hostname: String,
    pub cycles_per_call: u64,
    pub cycles_per_message_byte: u64,
    pub primary: bool,
}

#[derive(Debug, CandidType, Deserialize)]
pub struct RegisterProvider {
    pub chain_id: u64,
    pub hostname: String,
    pub credential_path: String,
    pub cycles_per_call: u64,
    pub cycles_per_message_byte: u64,
}

#[derive(Debug, CandidType, Deserialize)]
pub struct UpdateProvider {
    pub provider_id: u64,
    pub hostname: Option<String>,
    pub credential_path: Option<String>,
    pub cycles_per_call: Option<u64>,
    pub cycles_per_message_byte: Option<u64>,
    pub primary: Option<bool>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct Provider {
    pub provider_id: u64,
    pub owner: Principal,
    pub chain_id: u64,
    pub hostname: String,
    pub credential_path: String,
    pub cycles_per_call: u64,
    pub cycles_per_message_byte: u64,
    pub cycles_owed: u128,
    pub primary: bool,
}

#[derive(Debug, CandidType, Deserialize, Serialize, Clone)]
pub struct InitProvider {
    pub chain_id: u64,
    pub hostname: String,
    pub credential_path: String,
}

impl From<InitProvider> for RegisterProvider {
    fn from(value: InitProvider) -> Self {
        Self {
            chain_id: value.chain_id,
            hostname: value.hostname,
            credential_path: value.credential_path,
            cycles_per_call: 0,         // TODO:: Update values,
            cycles_per_message_byte: 0, // TODO:: Update values,
        }
    }
}

// These need to be powers of two so that they can be used as bit fields.
#[derive(Clone, Debug, PartialEq, CandidType, Deserialize)]
pub enum Auth {
    Admin = 0b0001,
    Rpc = 0b0010,
    RegisterProvider = 0b0100,
    FreeRpc = 0b1000,
}

pub async fn register_provider(
    provider: &InitProvider,
    context: &Rc<RefCell<dyn Context>>,
) -> Result<u64> {
    let eth_client = context.borrow().get_ic_eth_client();
    let provider_id = eth_client
        .update::<(RegisterProvider,), u64>("register_provider", (provider.clone().into(),))
        .await?;

    Ok(provider_id)
}

pub async fn check_if_provider_exists(
    provider: &InitProvider,
    context: &Rc<RefCell<dyn Context>>,
) -> Result<bool> {
    let eth_client = context.borrow().get_ic_eth_client();
    let providers = eth_client
        .query::<(InitProvider,), Vec<ProviderView>>("get_providers", (provider.clone(),))
        .await?;

    let val = providers
        .iter()
        .any(|p| p.chain_id == provider.chain_id && p.hostname == provider.hostname);

    Ok(val)
}

pub async fn check_and_register_provider(
    provider: &InitProvider,
    context: &Rc<RefCell<dyn Context>>,
) -> Result<()> {
    if !check_if_provider_exists(provider, context).await? {
        register_provider(provider, context).await?;
    }

    Ok(())
}
