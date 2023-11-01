use std::cell::RefCell;
use std::rc::Rc;

use candid::CandidType;
use did::{H160, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::abi::{Function, Param, ParamType, StateMutability};
use ethers_core::types::transaction::eip2718::TypedTransaction;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::context::Context;
use crate::error::{Error, Result};
use crate::http;

#[derive(Debug, CandidType, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct Provider {
    pub chain_id: u64,
    pub hostname: String,
}

pub async fn get_transaction(
    user_address: H160,
    provider: Provider,
    to: Option<H160>,
    value: U256,
    data: Vec<u8>,
    context: &Rc<RefCell<dyn Context>>,
) -> Result<ethers_core::types::Transaction> {
    // NOTE: this is a workaround for clippy "borrow reference held across await point"
    // For some reason clippy produces a false warning for the code
    // let context = context.borrow();
    // ...
    // drop(context); // before the first await point
    let signer = {
        let context = context.borrow();
        let signer = context.get_state().signer.get_oracle_signer(user_address);

        signer
    };

    let from = signer
        .get_address()
        .await
        .map_err(|e| Error::from(format!("failed to get address: {e}")))?;

    let nonce = http::call_jsonrpc(
        &provider.hostname,
        "eth_getTransactionCount",
        serde_json::json!([from, "latest"]),
        Some(8000),
    )
    .await?;

    let nonce: U256 = serde_json::from_value(nonce)?;

    let gas_price = http::call_jsonrpc(
        &provider.hostname,
        "eth_gasPrice",
        serde_json::Value::Null,
        Some(8000),
    )
    .await?;

    let gas_price: U256 = serde_json::from_value(gas_price)?;

    let gas = http::call_jsonrpc(
        &provider.hostname,
        "eth_estimateGas",
        serde_json::json!([{
            "from": from,
            "to": to,
            "value": value,
            "data": hex::encode(data.clone()),
        }]),
        Some(8000),
    )
    .await?;

    let gas: U256 = serde_json::from_value(gas)?;

    let mut transaction = ethers_core::types::Transaction {
        from: from.into(),
        to: to.map(Into::into),
        nonce: nonce.0,
        value: value.0,
        gas: gas.into(),
        gas_price: Some(gas_price.into()),
        input: data.into(),
        chain_id: Some(provider.chain_id.into()),
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

    Ok(transaction)
}

#[allow(deprecated)]
pub static UPDATE_PRICE: Lazy<Function> = Lazy::new(|| Function {
    name: "updatePrice".into(),
    inputs: vec![Param {
        name: "_price".into(),
        kind: ParamType::Int(256),
        internal_type: None,
    }],
    outputs: vec![],
    constant: None,
    state_mutability: StateMutability::NonPayable,
});

#[allow(deprecated)]
/// Returns the function selector for the given function name and parameters.
pub fn function_selector(name: &str, params: &[Param]) -> Function {
    Function {
        name: name.to_owned(),
        inputs: params.to_vec(),
        outputs: vec![],
        constant: None,
        state_mutability: StateMutability::NonPayable,
    }
}
