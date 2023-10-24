use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use evm_canister_client::EvmCanisterClient;
use ic_canister_client::{CanisterClient, IcCanisterClient};

use crate::contract::ContractService;
use crate::processor::{EvmTransactionProcessorImpl, EvmTransactionsProcessor};
use crate::state::{Settings, State};

/// Context to access the external traits
pub trait Context {
    /// Return a client to the EVM canister
    fn get_evm_client(&self) -> Rc<EvmCanisterClient<IcCanisterClient>>;

    /// Returns state reference
    fn get_state(&self) -> Ref<'_, State>;

    /// Returns mutable state reference
    fn mut_state(&self) -> RefMut<'_, State>;

    fn get_tx_processor(&self) -> Rc<dyn EvmTransactionsProcessor>;

    fn get_contract_service(&self) -> Rc<ContractService> {
        Rc::new(ContractService::default())
    }

    /// Resets context state to the default one
    fn reset(&mut self) {
        // self.mut_state().reset(Settings::default());
        // self.get_evm_canister().reset();
    }
}

#[derive(Default)]
pub struct ContextImpl<TxProcessor: EvmTransactionsProcessor> {
    state: RefCell<State>,
    tx_processor: Rc<TxProcessor>,
    contract_service: Rc<ContractService>,
}

impl ContextImpl<EvmTransactionProcessorImpl> {
    #[allow(dead_code)]
    pub fn get_tx_processor_impl(&self) -> Rc<EvmTransactionProcessorImpl> {
        self.tx_processor.clone()
    }
}

impl<TxProcessor: EvmTransactionsProcessor + 'static> Context for ContextImpl<TxProcessor> {
    fn get_evm_client(&self) -> Rc<EvmCanisterClient<IcCanisterClient>> {
        let client = IcCanisterClient::new(self.state.borrow().evm());
        let evm_client = EvmCanisterClient::new(client);

        Rc::new(evm_client)
    }

    fn get_state(&self) -> Ref<'_, State> {
        self.state.borrow()
    }

    fn mut_state(&self) -> RefMut<'_, State> {
        self.state.borrow_mut()
    }

    fn get_tx_processor(&self) -> Rc<dyn EvmTransactionsProcessor> {
        self.tx_processor.clone()
    }
}

pub fn get_base_context(context: &Rc<RefCell<impl Context + 'static>>) -> Rc<RefCell<dyn Context>> {
    let context: Rc<RefCell<dyn Context>> = context.clone();
    context
}
