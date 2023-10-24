use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use candid::CandidType;
use derive_more::From;
use did::codec::{decode, encode};
use did::{TransactionReceipt, H256};
use futures::future;
use ic_stable_structures::{
    get_memory_by_id, ChunkSize, MemoryId, SlicedStorable, StableUnboundedMap, Storable,
    UnboundedMapStructure,
};
use serde::Deserialize;

use crate::canister::ReserveAddressCallback;
use crate::context::Context;
use crate::contract::PriceFeedCreationCallback;
use crate::memory::{self, MEMORY_MANAGER, TX_CALLBACKS_MEMORY_ID};

/// Callback to process the result of the transaction
#[async_trait(?Send)]
pub trait TxResultCallback {
    /// Action on transaction processed
    async fn processed(self, result: TransactionReceipt, context: &Rc<RefCell<dyn Context>>);

    /// Transaction skipped (wasn't added to blockchain)
    async fn skipped(self, context: &Rc<RefCell<dyn Context>>);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, CandidType, Deserialize, From)]
pub enum TxCallback {
    PriceFeedCreation(PriceFeedCreationCallback),
    ReserveAddress(ReserveAddressCallback),
}

impl Storable for TxCallback {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        encode(self).into()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        decode(&bytes)
    }

    const BOUND: ic_stable_structures::Bound = ic_stable_structures::Bound::Unbounded;
}

impl SlicedStorable for TxCallback {
    const CHUNK_SIZE: ChunkSize = 64;
}

#[async_trait(?Send)]
impl TxResultCallback for TxCallback {
    async fn processed(self, result: TransactionReceipt, context: &Rc<RefCell<dyn Context>>) {
        match self {
            TxCallback::PriceFeedCreation(callback) => callback.processed(result, context),
            TxCallback::ReserveAddress(callback) => callback.processed(result, context),
        }
        .await
    }

    async fn skipped(self, context: &Rc<RefCell<dyn Context>>) {
        match self {
            TxCallback::PriceFeedCreation(callback) => callback.skipped(context),
            TxCallback::ReserveAddress(callback) => callback.skipped(context),
        }
        .await
    }
}

/// The component that registers a callback for the given EVMC transaction
pub trait EvmTransactionsProcessor {
    fn register_transaction(&self, tx_hash: H256, callback: TxCallback);

    fn reset(&self);
}

/// Status of the transaction in EVMC
enum TransactionStatus {
    /// Transaction still in pool or request failed
    Unknown,
    /// Transaction was skipped (isn't present both in pool and in blockchain)
    Skipped,
    /// Transaction is present in the blockchain
    Processed(TransactionReceipt),
}

#[derive(Default)]
pub struct EvmTransactionProcessorImpl {}

impl EvmTransactionProcessorImpl {
    /// Ping status of currently registered transactions
    pub async fn process_transactions(&self, context: &Rc<RefCell<dyn Context>>) {
        let mut futures: Vec<_> = TX_CALLBACKS.with(|callbacks| {
            callbacks
                .borrow()
                .iter()
                .map(move |(tx_hash, _)| {
                    let context = context.clone();
                    async move {
                        (
                            tx_hash.clone(),
                            Self::get_transaction_state(tx_hash, &context).await,
                        )
                    }
                })
                .map(Box::pin)
                .collect()
        });

        let extract_callback =
            |tx_hash| TX_CALLBACKS.with(|callbacks| callbacks.borrow_mut().remove(&tx_hash));
        while !futures.is_empty() {
            let ((tx_hash, result), _, remaining) = future::select_all(futures).await;
            futures = remaining;

            match result {
                TransactionStatus::Unknown => {}
                TransactionStatus::Skipped => {
                    if let Some(callback) = extract_callback(tx_hash) {
                        callback.skipped(context).await
                    }
                }
                TransactionStatus::Processed(receipt) => {
                    if let Some(callback) = extract_callback(tx_hash) {
                        callback.processed(receipt, context).await
                    }
                }
            }
        }
    }

    /// Get the transaction status from EVM canister
    async fn get_transaction_state(
        tx_hash: H256,
        context: &Rc<RefCell<dyn Context>>,
    ) -> TransactionStatus {
        let evm_client = context.borrow().get_evm_client();
        let tx_result = evm_client
            .eth_get_transaction_by_hash(tx_hash.clone())
            .await;
        // Check if the transaction is present either in blockchain or in the pool
        match tx_result {
            // transaction is still present
            Ok(Some(_)) => {
                if let Ok(receipt) = evm_client.eth_get_transaction_receipt(tx_hash).await {
                    match receipt {
                        // we have the the receipt
                        Ok(Some(receipt)) => TransactionStatus::Processed(receipt),
                        // no receipt yet or call failed
                        Ok(None) | Err(_) => TransactionStatus::Unknown,
                    }
                } else {
                    TransactionStatus::Unknown
                }
            }
            // no transaction in the blockchain or pool, it was not executed
            Ok(None) => TransactionStatus::Skipped,
            // request failed - retry
            Err(_) => TransactionStatus::Unknown,
        }
    }
}

#[async_trait]
impl EvmTransactionsProcessor for EvmTransactionProcessorImpl {
    fn register_transaction(&self, tx_hash: H256, callback: TxCallback) {
        TX_CALLBACKS.with(|callbacks| {
            _ = callbacks.borrow_mut().insert(&tx_hash, &callback);
        })
    }

    fn reset(&self) {
        TX_CALLBACKS.with(|callbacks| callbacks.borrow_mut().clear())
    }
}

thread_local! {
    static TX_CALLBACKS: RefCell<StableUnboundedMap<H256, TxCallback, memory::MemoryType>> =
        RefCell::new(StableUnboundedMap::new(get_memory_by_id(&MEMORY_MANAGER,TX_CALLBACKS_MEMORY_ID)));
}

#[cfg(test)]
pub mod tests {
    use std::collections::HashMap;
    use std::pin::Pin;
    use std::task::Poll;

    use did::H256;
    use futures::task::noop_waker;
    use futures::Future;

    use super::*;
    use crate::context::get_base_context;

    /// Executed future that doesn't actually have interrupt points.
    /// We need this function to avoid clippy warning about borrowing over await point.
    fn run_non_interrupted_future<Fut: Future>(mut f: Fut) -> Fut::Output {
        let f = unsafe { Pin::new_unchecked(&mut f) };
        let waker = noop_waker();
        let mut context = futures::task::Context::from_waker(&waker);
        match f.poll(&mut context) {
            Poll::Pending => panic!("future should be able to be executed at once"),
            Poll::Ready(res) => res,
        }
    }

    #[derive(Default)]
    pub struct EvmTransactionsProcessorMock {
        transactions: RefCell<HashMap<H256, TxCallback>>,
    }

    impl EvmTransactionsProcessorMock {
        pub fn processed<Ctx: Context + 'static>(
            &self,
            tx_hash: &H256,
            result: TransactionReceipt,
            context: &Rc<RefCell<Ctx>>,
        ) {
            let callback = self.transactions.borrow_mut().remove(tx_hash).unwrap();
            run_non_interrupted_future(callback.processed(result, &get_base_context(context)))
        }

        pub fn skipped<Ctx: Context + 'static>(&self, tx_hash: &H256, context: &Rc<RefCell<Ctx>>) {
            let callback = self.transactions.borrow_mut().remove(tx_hash).unwrap();
            run_non_interrupted_future(callback.skipped(&get_base_context(context)))
        }
    }

    impl EvmTransactionsProcessor for EvmTransactionsProcessorMock {
        fn register_transaction(&self, tx_hash: H256, callback: TxCallback) {
            self.transactions.borrow_mut().insert(tx_hash, callback);
        }

        fn reset(&self) {
            self.transactions.borrow_mut().clear();
        }
    }
}
