use std::cell::RefCell;
use std::rc::Rc;

use crate::context::Context;
use crate::error::{Error, Result};
use crate::gen;
use crate::processor::TxResultCallback;
use crate::state::Pair;
use async_trait::async_trait;
use candid::CandidType;
use derive_more::From;
use did::{Transaction, TransactionReceipt, H160, H256, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::abi::AbiEncode;
use ethers_core::abi::{Constructor, Param, ParamType, Token};
use ethers_core::types::transaction::eip2718::TypedTransaction;
use serde::Deserialize;

const DEFAULT_GAS_LIMIT: u64 = 30_000_000;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, CandidType, Deserialize, From)]
pub struct PriceFeedCreationCallback {
    pub pair_id: String,
}

impl PriceFeedCreationCallback {
    pub fn new(pair_id: String) -> Self {
        Self { pair_id }
    }
}

#[async_trait(?Send)]
impl TxResultCallback for PriceFeedCreationCallback {
    async fn processed(self, result: TransactionReceipt, context: &Rc<RefCell<dyn Context>>) {
        let ctx = context.borrow();
        let state = ctx.get_state();

        state
            .pair_storage
            .add_address(&self.pair_id, result.contract_address.unwrap_or_default())
    }

    async fn skipped(self, _context: &Rc<RefCell<dyn Context>>) {}
}

#[derive(Debug, Default, Clone)]
pub struct ContractService;

impl ContractService {
    /// Call the Aggregator contract in evmc to increase the currency price pairs supported by the aggregator

    pub async fn create_pair_feed(
        &self,
        context: &Rc<RefCell<dyn Context>>,
        pair: Pair,
        decimal: U256,
        description: String,
        version: U256,
    ) -> Result<H256> {
        let args = [
            Token::String(description),
            Token::Uint(decimal.0),
            Token::Uint(version.0),
        ];
        let constructor = Constructor {
            inputs: vec![
                Param {
                    name: "_description".into(),
                    kind: ParamType::String,
                    internal_type: None,
                },
                Param {
                    name: "_decimals".into(),
                    kind: ParamType::Uint(8),
                    internal_type: None,
                },
                Param {
                    name: "_version".into(),
                    kind: ParamType::Uint(256),
                    internal_type: None,
                },
            ],
        };

        let data = constructor
            .encode_input(gen::PRICEFEEDAPI_BYTECODE.to_vec(), &args)
            .map_err(|e| {
                Error::Internal(format!(
                    "Failed to encode constructor input: {}",
                    e.to_string()
                ))
            })?;

        let transaction = get_transaction(None, U256::zero(), data, &context).await?;

        let client = context.borrow().get_evm_client();

        let tx_hash = client.send_raw_transaction(transaction).await??;

        let callback = PriceFeedCreationCallback::new(pair.id());

        context
            .borrow()
            .get_tx_processor()
            .register_transaction(tx_hash.clone(), callback.into());

        Ok(tx_hash)
    }

    pub async fn update_pair_price(
        &self,
        context: &Rc<RefCell<dyn Context>>,
        price: u64,
        contract: H160,
    ) -> Result<()> {
        let data = gen::UpdatePriceCall {
            price: price.into(),
        }
        .encode();

        let transaction = get_transaction(Some(contract), U256::zero(), data, &context).await?;

        let client = context.borrow().get_evm_client();

        client.send_raw_transaction(transaction).await??;

        Ok(())
    }
}

async fn get_transaction(
    to: Option<H160>,
    value: U256,
    data: Vec<u8>,
    context: &Rc<RefCell<dyn Context>>,
) -> Result<Transaction> {
    // NOTE: this is a workaround for clippy "borrow reference held across await point"
    // For some reason clippy produces a false warning for the code
    // let context = context.borrow();
    // ...
    // drop(context); // before the first await point
    let (signer, gas_price, chain_id) = {
        let context = context.borrow();
        let signer = context.get_state().signer.get_transaction_signer();

        let evm_client = context.get_evm_client();

        let (gas_price, chain_id) =
            futures::future::join(evm_client.get_min_gas_price(), evm_client.eth_chain_id()).await;

        (signer, gas_price?, chain_id?)
    };

    let from = signer
        .get_address()
        .await
        .map_err(|e| Error::from(format!("failed to get address: {e}")))?;

    // Get the nonce

    let nonce = context
        .borrow()
        .get_evm_client()
        .account_basic(from.clone())
        .await?
        .nonce;

    let mut transaction = ethers_core::types::Transaction {
        from: from.into(),
        to: to.map(Into::into),
        nonce: nonce.0,
        value: value.0,
        gas: DEFAULT_GAS_LIMIT.into(),
        gas_price: Some(gas_price.into()),
        input: data.into(),
        chain_id: Some(chain_id.into()),
        ..Default::default()
    };
    let typed_transaction: TypedTransaction = (&transaction).into();

    let signature = signer
        .sign_transaction(&typed_transaction)
        .await
        .map_err(|e| Error::from(format!("failed to sign transaction: {e}")))?;

    transaction.r = signature.r.into();
    transaction.s = signature.s.into();
    transaction.v = signature.v.into();

    transaction.hash = transaction.hash();

    Ok(transaction.into())
}
