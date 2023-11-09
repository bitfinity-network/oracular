use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::str::FromStr;
use std::time::Duration;

use candid::{CandidType, Principal};
use did::{H160, H256, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::abi::ethabi;
use ethers_core::types::Signature;
use futures::TryFutureExt;
use ic_canister::{generate_idl, init, query, update, Canister, Idl, PreUpdate};
use ic_exports::ic_cdk;
use ic_exports::ic_cdk::api::management_canister::http_request::{
    HttpResponse as MHttpResponse, TransformArgs,
};
use ic_exports::ic_cdk_timers::TimerId;
use ic_exports::ic_kit::ic;
use ic_log::{init_log, LogSettings};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use serde_json::Value;

use crate::context::{get_base_context, Context, ContextImpl};
use crate::error::{Error, Result};
use crate::http::{self, transform, HttpRequest, HttpResponse};
use crate::log::LoggerConfigService;
use crate::provider::{self, get_transaction, Provider, UPDATE_PRICE};
use crate::state::oracle_storage::OracleMetadata;
use crate::state::{Settings, State, UpdateOracleMetadata};

/// Type alias for the shared mutable context implementation we use in the canister
type SharedContext = Rc<RefCell<ContextImpl>>;

#[derive(Clone, Default)]
pub struct ContextWrapper(pub SharedContext);

#[derive(Canister, Clone)]
pub struct Oracular {
    #[id]
    pub id: Principal,
    pub context: ContextWrapper,
}

impl PreUpdate for Oracular {}

/// The init data that will be used to initialize the canister
#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct InitData {
    /// The owner of the canister
    pub owner: Principal,
    #[serde(default)]
    pub log_settings: Option<LogSettings>,
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

    pub fn logger_config_service(&self) -> LoggerConfigService {
        LoggerConfigService::default()
    }

    #[init]
    pub fn init(&mut self, data: InitData) {
        match init_log(&data.log_settings.clone().unwrap_or_default()) {
            Ok(logger_config) => self.logger_config_service().init(logger_config),
            Err(err) => {
                ic_exports::ic_cdk::println!("error configuring the logger. Err: {:?}", err)
            }
        }

        info!("starting oracular canister");

        let settings = Settings::new(data.owner);

        check_anonymous_principal(data.owner).expect("invalid owner");

        self.with_state_mut(|state| state.reset(settings));
    }

    /// Returns the owner of the canister
    #[query]
    pub fn owner(&self) -> Principal {
        self.with_state(|state| state.owner())
    }

    /// Sets the owner of the canister
    #[update]
    pub fn set_owner(&mut self, owner: Principal) -> Result<()> {
        // Check anonymous principal
        check_anonymous_principal(owner)?;
        self.check_owner(ic::caller())?;

        self.with_state_mut(|state| state.set_owner(owner));
        Ok(())
    }

    /// Updates the runtime configuration of the logger with a new filter in the same form as the `RUST_LOG`
    /// environment variable.
    /// Example of valid filters:
    /// - info
    /// - debug,crate1::mod1=error,crate1::mod2,crate2=debug
    #[update]
    pub fn set_logger_filter(&mut self, filter: String) -> Result<()> {
        self.check_owner(ic::caller())?;
        self.logger_config_service().set_logger_filter(&filter)?;

        debug!("updated logger filter to {filter}");

        Ok(())
    }

    /// Gets the logs
    /// - `count` is the number of logs to return
    #[update]
    pub fn ic_logs(&self, count: usize) -> Result<Vec<String>> {
        self.check_owner(ic::caller())?;

        Ok(ic_log::take_memory_records(count))
    }

    /// Get all the oracles created
    #[query]
    pub fn get_all_oracles(&self) -> Vec<(H160, BTreeMap<H160, OracleMetadata>)> {
        self.with_state(|state| state.oracle_storage().get_oracles())
    }

    /// Returns the list of oracles for the given user
    #[query]
    pub fn get_user_oracles(&self, user_address: H160) -> Result<Vec<(H160, OracleMetadata)>> {
        let oracles =
            self.with_state(|state| state.oracle_storage().get_user_oracles(user_address))?;

        Ok(oracles)
    }

    /// Returns the address of the sender of the transaction using
    /// the management canister
    #[update]
    pub async fn get_address(&self, address: H160) -> Result<H160> {
        let signer = {
            self.context
                .0
                .borrow()
                .get_state()
                .signer
                .get_oracle_signer(address)
        };

        Ok(signer.get_address().await?)
    }

    /// Returns the metadata of the given oracle
    ///
    /// # Arguments
    /// * `contract_address` - The address of the contract that will be fetched
    /// * `user_address` - The address of the user that created the oracle
    #[query]
    pub fn get_oracle_metadata(
        &self,
        user_address: H160,
        contract_address: H160,
    ) -> Result<OracleMetadata> {
        let metadata = self.with_state(|state| {
            state
                .oracle_storage()
                .get_oracle_by_address(user_address, contract_address)
        })?;

        Ok(metadata)
    }

    /// Recovers the public key from the given message and signature
    /// and adds the signer to the list of signers
    ///
    /// This is used for the users to sign the transactions using the threshold
    /// ECDSA
    pub fn recover_pubkey(message: String, signature: String) -> Result<H160> {
        let signature = Signature::from_str(&signature).map_err(|e| {
            Error::Internal(format!("failed to parse signature: {:?}", e.to_string()))
        })?;

        let address = signature.recover(message).map_err(|e| {
            Error::Internal(format!("failed to recover public key: {:?}", e.to_string()))
        })?;

        Ok(address.into())
    }

    #[query]
    fn http_request(&self, req: HttpRequest) -> HttpResponse {
        if req.method.as_ref() != "POST" {
            return HttpResponse::error(400, "Method not allowed".to_string());
        }

        HttpResponse {
            status_code: 204,
            headers: HashMap::new(),
            body: ByteBuf::new(),
            upgrade: Some(true),
        }
    }

    #[update]
    pub async fn http_request_update(&self, req: HttpRequest) -> HttpResponse {
        log::debug!("start http_request_update: {:?}", req);

        let body = serde_json::from_slice::<Value>(&req.body)
            .map_err(|e| Error::Http(format!("serde_json err: {}", e)))
            .and_then(|body| {
                let message = body
                    .get("message")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Http("message is missing".to_string()))?;
                let signature = body
                    .get("signature")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Http("signature is missing".to_string()))?;
                Ok((message.to_string(), signature.to_string()))
            });

        match body {
            Ok((message, signature)) => {
                let address =
                    Self::recover_pubkey(message, signature).expect("failed to recover public key");

                let signer = self
                    .context
                    .0
                    .borrow()
                    .get_state()
                    .signer
                    .get_oracle_signer(address);

                let address = signer
                    .get_address()
                    .await
                    .map_err(|e| Error::Internal(format!("failed to get address: {e}")));

                match address {
                    Ok(address) => HttpResponse::new(
                        200,
                        HashMap::from([("content-type".into(), "text/plain".into())]),
                        ByteBuf::from(address.0.as_bytes()),
                        None,
                    ),
                    Err(e) => {
                        log::error!("failed to get address: {:?}", e.to_string());
                        HttpResponse::error(400, e.to_string())
                    }
                }
            }
            Err(e) => {
                log::error!("failed to recover public key: {:?}", e.to_string());
                HttpResponse::error(400, e.to_string())
            }
        }
    }

    /// Requirements for Http outcalls, used to ignore small differences in the data obtained
    /// by different nodes of the IC subnet to reach a consensus, more info:
    /// https://internetcomputer.org/docs/current/developer-docs/integrations/http_requests/http_requests-how-it-works#transformation-function
    #[query]
    fn transform(&self, raw: TransformArgs) -> MHttpResponse {
        transform(raw)
    }

    /// Updates the metadata of the given oracle
    ///
    /// # Arguments
    /// * `contract_address` - The address of the contract that will be updated
    /// * `metadata` - The metadata that will be used to update the oracle
    ///
    /// # Errors
    /// * If the caller is not the owner
    /// * If the metadata is None
    /// * If the oracle is not found
    ///
    /// # Note
    /// When we update the metadata, we also update the timer that will be used
    /// to update the price of the oracle
    #[update]
    pub async fn update_oracle_metadata(
        &self,
        user_address: H160,
        contract_address: H160,
        metadata: UpdateOracleMetadata,
    ) -> Result<()> {
        // If all the values are None, then return an error
        if metadata.is_none() {
            return Err(Error::Internal(
                "At least one of the metadata fields must be set".to_string(),
            ));
        }

        let old_md = self.with_state(|state| {
            state
                .oracle_storage()
                .get_oracle_by_address(user_address.0.into(), contract_address.clone())
        })?;

        // Check the old owner of the oracle
        if old_md.owner != user_address {
            return Err(Error::Internal(
                "caller is not the owner of the oracle".to_string(),
            ));
        }

        let timer_id = self.with_state(|state| {
            state
                .oracle_storage()
                .get_timer_id_by_address(user_address.0.into(), contract_address.clone())
        })?;

        ic_exports::ic_cdk_timers::clear_timer(timer_id);

        let timer_id = Self::init_price_timer(
            get_base_context(&self.context.0),
            user_address.0.into(),
            metadata.timestamp.unwrap_or(old_md.timer_interval),
            metadata.origin.clone().unwrap_or(old_md.origin),
            metadata.evm.clone().unwrap_or(old_md.evm),
        )
        .await?;

        self.with_state_mut(|state| {
            state.mut_oracle_storage().update_oracle_metadata(
                user_address,
                contract_address,
                Some(timer_id),
                metadata,
            )
        })?;

        Ok(())
    }

    #[update]
    pub fn delete_oracle(&mut self, user_address: H160, contract_address: H160) -> Result<()> {
        // Get the owner
        let owner = self.with_state(|state| {
            state
                .oracle_storage()
                .get_oracle_owner(user_address.0.into(), contract_address.clone())
        })?;

        if owner != user_address {
            return Err(Error::Internal(
                "caller is not the owner of the oracle".to_string(),
            ));
        }

        let timer_id = self.with_state(|state| {
            state
                .oracle_storage()
                .get_timer_id_by_address(user_address.0.into(), contract_address.clone())
        })?;

        ic_exports::ic_cdk_timers::clear_timer(timer_id);

        self.with_state_mut(|state| {
            state
                .mut_oracle_storage()
                .remove_oracle_by_address(user_address, contract_address)
        })?;

        Ok(())
    }

    /// Creates an oracle that will fetch the data from the given URL
    /// and will update the price of the given contract
    /// every `timestamp` seconds
    ///
    /// # Arguments
    /// * `origin` - The origin of the data that will be used to update the price
    /// * `timestamp` - The interval in seconds that will be used to update the price
    /// * `destination` - The destination of the data that will be used to update the price
    ///
    #[update]
    pub async fn create_oracle(
        &mut self,
        user_address: H160,
        origin: Origin,
        timestamp: u64,
        destination: EvmDestination,
    ) -> Result<()> {
        log::debug!("creating new oracle: {:?}", origin);

        // Start the timer
        let timer_id = Self::init_price_timer(
            get_base_context(&self.context.0),
            user_address.0.into(),
            timestamp,
            origin.clone(),
            destination.clone(),
        )
        .await?;

        // Save the metadata
        self.with_state_mut(|state| {
            state.mut_oracle_storage().add_oracle(
                user_address,
                origin,
                timestamp,
                timer_id,
                destination,
            )
        });

        log::debug!("oracle created successfully ");

        Ok(())
    }

    /// Initializes the timer that will be used to update the price
    pub async fn init_price_timer(
        context: Rc<RefCell<dyn Context>>,
        user_address: H160,
        timestamp: u64,
        origin: Origin,
        evm: EvmDestination,
    ) -> Result<TimerId> {
        let timer_id = ic_exports::ic_cdk_timers::set_timer_interval(
            Duration::from_secs(timestamp),
            move || {
                let future = Self::send_transaction(
                    origin.clone(),
                    user_address.0.into(),
                    evm.clone(),
                    context.clone(),
                )
                .unwrap_or_else(|e| {
                    log::error!("failed to send transaction: {:?}", e.to_string());
                });

                ic_cdk::spawn(future);
            },
        );

        Ok(timer_id)
    }

    /// Sends a transaction to the EVM
    async fn send_transaction(
        origin: Origin,
        user_address: H160,
        evm_destination: EvmDestination,
        context: Rc<RefCell<dyn Context>>,
    ) -> Result<()> {
        log::debug!(
            "Updating oracle price: user_address :{} origin: {:?} evm_destination: {:?} ",
            user_address,
            origin,
            evm_destination
        );

        let response = match origin {
            Origin::Evm(EvmOrigin {
                ref provider,
                ref target_address,
                ref method,
            }) => {
                let data = provider::function_selector(method, &[]).encode_input(&[])?;

                let data_hex = did::Bytes::from(data).to_hex_str();
                let params = serde_json::json!([{
                    "to": target_address,
                    "data": data_hex,
                }]);

                let res =
                    http::call_jsonrpc(&provider.hostname, "eth_call", params, Some(80000)).await?;

                serde_json::from_value::<U256>(res)?
            }
            Origin::Http(HttpOrigin {
                ref url,
                ref json_path,
            }) => http::get_price(url, json_path).await?,
        };

        let (hostname, chain_id) = (
            evm_destination.provider.hostname,
            evm_destination.provider.chain_id,
        );

        let data = UPDATE_PRICE.encode_input(&[ethabi::Token::Int(response.into())])?;

        let provider = Provider {
            chain_id,
            hostname: hostname.to_owned(),
        };

        let transaction = get_transaction(
            user_address,
            provider,
            Some(evm_destination.contract.0.into()),
            U256::zero(),
            data,
            &context,
        )
        .await?;

        let params = serde_json::json!([format!("0x{}", hex::encode(transaction.rlp()))]);

        let tx_hash =
            http::call_jsonrpc(&hostname, "eth_sendRawTransaction", params, Some(80000)).await?;

        let tx_hash = serde_json::from_value::<H256>(tx_hash)?;

        log::debug!("transaction hash: {:?}", tx_hash);

        Ok(())
    }

    fn check_owner(&self, caller: Principal) -> Result<()> {
        let owner = self.with_state(|state| state.owner());
        if caller != owner {
            return Err(Error::Internal("caller is not the owner".to_string()));
        }

        Ok(())
    }

    /// Returns candid IDL.
    /// This should be the last fn to see previous endpoints in macro.
    pub fn idl() -> Idl {
        generate_idl!()
    }
}

/// This is the origin of the data that will be used to update the price
#[derive(Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum Origin {
    /// EVM origin
    Evm(EvmOrigin),
    /// HTTP origin
    Http(HttpOrigin),
}

/// EVM origin data
#[derive(Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvmOrigin {
    /// The EVM provider that will be used to fetch the data
    pub provider: Provider,
    /// The address of the contract that will be called
    pub target_address: H160,
    /// The method that will be called on the contract
    pub method: String,
}

/// HTTP origin data that will be used to fetch the data from the given URL
#[derive(Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpOrigin {
    /// The URL that will be used to fetch the data
    pub url: String,
    /// The JSON path that will be used to extract the data
    pub json_path: String,
}

/// This is the destination of the data that will be used to update the price
#[derive(Debug, Clone, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvmDestination {
    /// The address of the contract that will be called
    pub contract: H160,
    /// The EVM provider that will be used to fetch the data
    pub provider: Provider,
}

/// inspect function to check whether the provided principal is anonymous
fn check_anonymous_principal(principal: Principal) -> Result<()> {
    if principal == Principal::anonymous() {
        return Err(Error::Internal("Principal is anonymous".to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use candid::Principal;
    use ic_canister::{canister_call, Canister};
    use ic_exports::ic_kit::mock_principals::alice;
    use ic_exports::ic_kit::MockContext;

    use super::*;
    use crate::canister::Oracular;

    pub fn oracular_principal_mock() -> Principal {
        const MOCK_PRINCIPAL: &str = "sgymv-uiaaa-aaaaa-aaaia-cai";
        Principal::from_text(MOCK_PRINCIPAL).expect("valid principal")
    }

    async fn init_canister<'a>() -> (Oracular, &'a mut MockContext) {
        let ctx = MockContext::new().inject();

        let mut canister = Oracular::from_principal(oracular_principal_mock());

        canister_call!(
            canister.init(InitData {
                owner: Principal::management_canister(),
                log_settings: None,
            }),
            ()
        )
        .await
        .unwrap();

        (canister, ctx)
    }

    #[tokio::test]
    async fn test_set_owner_anonymous() {
        let (mut canister, ctx) = init_canister().await;

        let res = canister_call!(canister.set_owner(Principal::anonymous()), ())
            .await
            .unwrap();

        assert!(res.is_err());

        ctx.update_id(Principal::management_canister());

        let res = canister_call!(canister.set_owner(alice()), ())
            .await
            .unwrap();

        assert!(res.is_ok());
    }

    #[test]
    fn test_recover_pub_key_with_correct_payload() {
        let message = "Testing".to_string();
        let signature =
        "0x4bce59ed739b43e739f304cb790cacde57b800aa712dde352cc8aa4f4727979d3849a8c52f59c34083f5060b4f1630ad7d34902a68ae216431332f27b830953b1b".to_string();

        let expected_address =
            H160::from_hex_str("0xE757Bd3f57C51D2068742d0CEA6f49D38d567310").unwrap();

        let address = Oracular::recover_pubkey(message, signature).unwrap();

        assert_eq!(address, expected_address);
    }

    #[test]
    fn test_recover_pub_key_with_incorrect_payload() {
        let message = "Testing 123".to_string();
        let signature =
        "0x4bce59ed739b43e739f304cb790cacde57b800aa712dde352cc8aa4f4727979d3849a8c52f59c34083f5060b4f1630ad7d34902a68ae216431332f27b830953b1b".to_string();

        let expected_address =
            H160::from_hex_str("0xE757Bd3f57C51D2068742d0CEA6f49D38d567310").unwrap();

        let address = Oracular::recover_pubkey(message, signature).unwrap();

        assert_ne!(address, expected_address);
    }
}
