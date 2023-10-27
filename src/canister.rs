use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use std::time::Duration;

use candid::{CandidType, Principal};
use did::{H160, U256};
use ethers_core::types::Signature;
use ic_canister_client::CanisterClient;

use futures::TryFutureExt;

use ic_exports::ic_cdk;
use ic_exports::ic_cdk::api::management_canister::http_request::{HttpResponse, TransformArgs};

use ethers_core::abi::ethabi;

use ic_canister::{generate_idl, init, query, update, Canister, Idl, PreUpdate};
use ic_exports::ic_cdk_timers::TimerId;
use ic_exports::ic_kit::ic;
use serde::{Deserialize, Serialize};

use crate::context::{get_base_context, get_transaction, Context, ContextImpl};

use crate::error::{Error, Result};
use crate::eth_rpc::{self, InitProvider, ProviderView, RegisterProvider, Source};
use crate::http::{self, transform};
use crate::state::{Settings, State, UpdateOracleMetadata};

/// ETH RPC canister ID
/// 6yxaq-riaaa-aaaap-abkpa-cai
pub const ETH_RPC_CANISTER: Principal = Principal::from_slice(&[0, 0, 0, 0, 1, 224, 10, 158, 1, 1]);

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
#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct InitData {
    /// The owner of the canister
    pub owner: Principal,
    /// The ETH RPC canister
    pub ic_eth_rpc: Option<Principal>,
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

    #[init]
    pub fn init(&mut self, data: InitData) {
        let eth_rpc = data.ic_eth_rpc.unwrap_or(ETH_RPC_CANISTER);
        let settings = Settings::new(data.owner, eth_rpc);

        check_anonymous_principal(data.owner).expect("invalid owner");
        check_anonymous_principal(eth_rpc).expect("invalid ic_eth");

        self.with_state_mut(|state| state.reset(settings));
    }

    /// Returns the address of the signer
    #[update]
    pub async fn get_user_address(&self, principal: Principal) -> Result<H160> {
        let signer = self.with_state(|state| state.signer().clone());

        signer.get_signer_address(principal)
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

    /// Sets the ETH principal
    #[update]
    pub fn set_eth_principal(&mut self, eth_principal: Principal) -> Result<()> {
        check_anonymous_principal(eth_principal)?;
        self.check_owner(ic::caller())?;

        self.with_state_mut(|state| state.set_ic_eth(eth_principal));
        Ok(())
    }

    #[query]
    pub fn eth_principal(&self) -> Principal {
        self.with_state(|state| state.ic_eth())
    }

    /// Recovers the public key from the given message and signature
    /// and adds the signer to the list of signers
    ///
    /// This is used for the users to sign the transactions using the threshold
    /// ECDSA
    #[update]
    pub fn recover_pubkey(&self, message: String, signature: String) -> Result<H160> {
        let signature = Signature::from_str(&signature).map_err(|e| {
            Error::Internal(format!("failed to parse signature: {:?}", e.to_string()))
        })?;

        let address = signature.recover(message).map_err(|e| {
            Error::Internal(format!("failed to recover public key: {:?}", e.to_string()))
        })?;

        let signer = self.with_state(|state| state.signer().clone());
        signer.add_signer(ic::caller(), address.into());

        Ok(address.into())
    }

    /// Requirements for Http outcalls, used to ignore small differences in the data obtained
    /// by different nodes of the IC subnet to reach a consensus, more info:
    /// https://internetcomputer.org/docs/current/developer-docs/integrations/http_requests/http_requests-how-it-works#transformation-function
    #[query]
    fn transform(&self, raw: TransformArgs) -> HttpResponse {
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
        contract_address: H160,
        metadata: UpdateOracleMetadata,
    ) -> Result<()> {
        let caller = ic::caller();
        check_anonymous_principal(caller)?;

        // If all the values are None, then return an error
        if metadata.is_none() {
            return Err(Error::Internal(
                "At least one of the metadata fields must be set".to_string(),
            ));
        }

        let old_md = self.with_state(|state| {
            state
                .oracle_storage()
                .get_oracle_by_address(caller, contract_address.clone())
        })?;
        ic_exports::ic_cdk_timers::clear_timer(old_md.timer_id);

        let timer_id = Self::init_price_timer(
            get_base_context(&self.context.0),
            caller,
            metadata.timestamp.unwrap_or(old_md.timer_interval),
            metadata.origin.clone().unwrap_or(old_md.origin),
            metadata.evm.clone().unwrap_or(old_md.evm),
        )
        .await?;

        self.with_state_mut(|state| {
            state.mut_oracle_storage().update_oracle_metadata(
                caller,
                contract_address,
                Some(timer_id),
                metadata,
            )
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
        origin: Origin,
        timestamp: u64,
        destination: EvmDestination,
    ) -> Result<()> {
        let caller = ic::caller();
        check_anonymous_principal(caller)?;

        if let Origin::Evm(EvmOrigin { ref provider, .. }) = origin {
            // Check and register the provider
            eth_rpc::check_and_register_provider(provider, &get_base_context(&self.context.0))
                .await?;
        }

        // Register the destination provider
        eth_rpc::check_and_register_provider(
            &destination.provider,
            &get_base_context(&self.context.0),
        )
        .await?;

        // Start the timer
        let timer_id = Self::init_price_timer(
            get_base_context(&self.context.0),
            caller,
            timestamp,
            origin.clone(),
            destination.clone(),
        )
        .await?;

        // Save the metadata
        self.with_state_mut(|state| {
            state
                .mut_oracle_storage()
                .add_oracle(caller, origin, timestamp, timer_id, destination)
        });

        Ok(())
    }

    /// Initializes the timer that will be used to update the price
    pub async fn init_price_timer(
        context: Rc<RefCell<dyn Context>>,
        principal: Principal,
        timestamp: u64,
        origin: Origin,
        evm: EvmDestination,
    ) -> Result<TimerId> {
        let timer_id = ic_exports::ic_cdk_timers::set_timer_interval(
            Duration::from_secs(timestamp),
            move || {
                let future =
                    Self::send_transaction(origin.clone(), principal, context.clone(), evm.clone())
                        .unwrap_or_else(|e| {
                            ic::print(format!("failed to send transaction: {:?}", e.to_string()))
                        });

                ic_cdk::spawn(future);
            },
        );

        Ok(timer_id)
    }

    /// Sends a transaction to the EVM
    async fn send_transaction(
        origin: Origin,
        principal: Principal,
        context: Rc<RefCell<dyn Context>>,
        evm_destination: EvmDestination,
    ) -> Result<()> {
        let eth_client = context.borrow().get_ic_eth_client();

        let response = match origin {
            Origin::Evm(EvmOrigin {
                ref provider,
                ref target_address,
                ref method,
            }) => {
                let json_rpc_payload = format!(
                    r#"[{{"jsonrpc":"2.0","id":"67","method":"eth_call","params":[{{"to":"0x{}","data":"0x{:?}"}},"latest"]}}]"#,
                    H160::from(target_address.0),
                    ethabi::encode(&[ethabi::Token::String(method.to_owned())]).to_vec()
                );

                let source = Source::Service {
                    hostname: provider.hostname.clone(),
                    chain_id: Some(provider.chain_id),
                };

                let res = eth_client
                    .update::<(Source, String, u64), String>(
                        "request",
                        (source, json_rpc_payload, 80000),
                    )
                    .await?;

                U256::from_hex_str(&res)?
            }
            Origin::Http(HttpOrigin {
                ref url,
                ref json_path,
            }) => http::get_price(url, json_path).await?,
        };

        let (ref hostname, ref chain_id) = match origin {
            Origin::Evm(_) => (
                &evm_destination.provider.hostname,
                Some(evm_destination.provider.chain_id),
            ),
            Origin::Http(_) => (
                &evm_destination.provider.hostname,
                Some(evm_destination.provider.chain_id),
            ),
        };

        let source = Source::Service {
            hostname: hostname.to_string(),
            chain_id: *chain_id,
        };

        let data = ethabi::encode(&[
            ethabi::Token::String("updatePrice(uint256)".to_string()),
            ethabi::Token::Tuple(vec![ethabi::Token::Uint(response.into())]),
        ]);

        let transaction: ethers_core::types::Transaction = get_transaction(
            principal,
            source.clone(),
            Some(evm_destination.contract.0.into()),
            U256::zero(),
            data,
            &context,
        )
        .await?
        .into();

        let json_rpc_payload = format!(
            r#"[{{"jsonrpc":"2.0","id":"67","method":"eth_sendRawTransaction","params":["0x{}"]}}]"#,
            hex::encode(transaction.rlp())
        );

        let response = eth_client
            .update::<(Source, String, u64), String>("request", (source, json_rpc_payload, 80000))
            .await?;

        ic::print(format!("response: {:?}", response));
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
#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub enum Origin {
    /// EVM origin
    Evm(EvmOrigin),
    /// HTTP origin
    Http(HttpOrigin),
}

/// EVM origin data
#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct EvmOrigin {
    /// The EVM provider that will be used to fetch the data
    pub provider: InitProvider,
    /// The address of the contract that will be called
    pub target_address: H160,
    /// The method that will be called on the contract
    pub method: String,
}

/// HTTP origin data that will be used to fetch the data from the given URL
#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct HttpOrigin {
    /// The URL that will be used to fetch the data
    pub url: String,
    /// The JSON path that will be used to extract the data
    pub json_path: String,
}

/// This is the destination of the data that will be used to update the price
#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct EvmDestination {
    /// The address of the contract that will be called
    pub contract: H160,
    /// The EVM provider that will be used to fetch the data
    pub provider: InitProvider,
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
    use crate::canister::Oracular;

    use candid::Principal;
    use ic_canister::{canister_call, Canister};
    use ic_exports::ic_kit::mock_principals::alice;
    use ic_exports::ic_kit::MockContext;

    use super::*;

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
                ic_eth_rpc: None
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

    #[tokio::test]
    async fn test_set_eth_principal_anonymous() {
        let (mut canister, ctx) = init_canister().await;

        let res = canister_call!(canister.set_eth_principal(Principal::anonymous()), ())
            .await
            .unwrap();

        assert!(res.is_err());

        ctx.update_id(Principal::management_canister());

        let res = canister_call!(canister.set_eth_principal(alice()), ())
            .await
            .unwrap();

        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_set_eth_principal() {
        let (mut canister, ctx) = init_canister().await;

        ctx.update_id(Principal::management_canister());

        let res = canister_call!(canister.set_eth_principal(alice()), ())
            .await
            .unwrap();

        assert!(res.is_ok());

        let res = canister_call!(canister.eth_principal(), ()).await.unwrap();

        assert_eq!(res, alice());
    }

    // #[tokio::test]
    // async fn create_oracle() {
    //     let (mut canister, ctx) = init_canister().await;

    //     ctx.update_id(Principal::management_canister());

    //     let res =
    //         canister_call!(canister.create_oracle(
    //         Origin::Http(
    //             "https://api.coingecko.com/api/v3/simple/price?ids=ethereum&vs_currencies=usd"
    //                 .to_string()
    //         ),
    //         "latestPrice".to_string(),
    //         Some("price".to_string()),
    //         300,
    //         EvmDestination {
    //             contract: H160::from_slice(&[5]),
    //             provider: InitProvider {
    //                 hostname: "https://api.coingecko.com/api/v3".to_string(),
    //                 chain_id: 1,
    //                 credential_path: "/path/to/credential".to_string(),
    //             }
    //         }
    //     ),())
    //         .await
    //         .unwrap();
    // }
}
