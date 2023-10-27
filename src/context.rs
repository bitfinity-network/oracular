use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use crate::error::{Error, Result};
use crate::eth_rpc::Source;
use crate::state::{Settings, State};

use did::{Transaction, H160, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::types::transaction::eip2718::TypedTransaction;
use ic_canister_client::CanisterClient;
use ic_canister_client::IcCanisterClient;

/// Context to access the external traits
pub trait Context {
    /// Returns state reference
    fn get_state(&self) -> Ref<'_, State>;

    /// Returns mutable state reference
    fn mut_state(&self) -> RefMut<'_, State>;

    fn get_ic_eth_client(&self) -> Rc<IcCanisterClient>;

    /// Resets context state to the default one
    fn reset(&mut self) {
        self.mut_state().reset(Settings::default());
    }
}

#[derive(Default)]
pub struct ContextImpl {
    state: RefCell<State>,
}

impl Context for ContextImpl {
    fn get_state(&self) -> Ref<'_, State> {
        self.state.borrow()
    }

    fn mut_state(&self) -> RefMut<'_, State> {
        self.state.borrow_mut()
    }

    fn get_ic_eth_client(&self) -> Rc<IcCanisterClient> {
        Rc::new(IcCanisterClient::new(self.state.borrow().ic_eth()))
    }
}

pub fn get_base_context(context: &Rc<RefCell<impl Context + 'static>>) -> Rc<RefCell<dyn Context>> {
    let context: Rc<RefCell<dyn Context>> = context.clone();
    context
}

const DEFAULT_GAS_LIMIT: u64 = 30_000_000;

pub async fn get_transaction(
    user_address: H160,
    source: Source,
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
    let (signer, eth_client) = {
        let context = context.borrow();
        let signer = context.get_state().signer.get_oracle_signer(user_address);
        let eth_client = context.get_ic_eth_client();

        (signer, eth_client)
    };

    let from = signer
        .get_address()
        .await
        .map_err(|e| Error::from(format!("failed to get address: {e}")))?;

    let json_rpc_payload = format!(
        r#"[{{"jsonrpc":"2.0","id":"67","method":"eth_getTransactionCount","params":["{:?}"]}}]"#,
        from
    );

    // Get the nonce
    let nonce = eth_client
        .update::<(Source, String, u64), String>(
            "request",
            (source.clone(), json_rpc_payload, 80000),
        )
        .await?;

    let nonce = U256::from_hex_str(&nonce)?;

    let json_rpc_payload =
        r#"[{"jsonrpc":"2.0","id":"67","method":"eth_gasPrice","params":[""]}]"#.to_string();

    // Get the nonce
    let gas_price = eth_client
        .update::<(Source, String, u64), String>(
            "request",
            (source.clone(), json_rpc_payload, 80000),
        )
        .await?;

    let gas_price = U256::from_hex_str(&gas_price)?;

    let chain_id = match source {
        Source::Service { chain_id, .. } => chain_id.unwrap_or_default(),
        _ => unreachable!(),
    };
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
