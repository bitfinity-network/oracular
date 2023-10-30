use candid::Principal;
use did::H160;
use ic_canister_client::CanisterClient;
use ic_exports::ic_kit::mock_principals::alice;
use oracular::canister::{EvmDestination, HttpOrigin, Origin};
use oracular::error::Result;
use oracular::provider::Provider;

use crate::context::state_machine::StateMachineTestContext;
use crate::context::TestContext;

#[tokio::test]
async fn set_owner_access() {
    let ctx = StateMachineTestContext::reset_and_lock().await;
    let client = ctx.client(ctx.canisters.oracular, ctx.admin_name());

    let res = client
        .update::<(Principal,), Result<()>>("set_owner", (alice(),))
        .await
        .unwrap();

    assert!(res.is_ok());

    // Assert owner is set
    let res = client.query::<(), Principal>("owner", ()).await.unwrap();

    assert_eq!(res, alice());
}

#[tokio::test]
async fn test_create_oracle_http_origin() {
    let ctx = StateMachineTestContext::reset_and_lock().await;

    let client = ctx.client(ctx.canisters.oracular, ctx.admin_name());

    client
        .update::<(), Result<()>>("authorize_oracular", ())
        .await
        .unwrap()
        .unwrap();

    let origin = Origin::Http(HttpOrigin {
        url: String::from("https://api.coinbase.com/v2/prices/BTC-ETH/spot"),
        json_path: String::from("data.amount"),
    });

    let destination = EvmDestination {
        contract: H160::from_hex_str("0x637F877db257ccba80B1fe06b0bEA039cd92C736").unwrap(),
        provider: Provider {
            chain_id: 355113,
            hostname: "https://127.0.0.1:8545".to_string(),
        },
    };

    let res = client
        .update::<(Origin, u64, EvmDestination), Result<()>>(
            "create_oracle",
            (origin, 1, destination),
        )
        .await
        .unwrap();
    ctx.advance_time(std::time::Duration::from_secs(1)).await;

    assert!(res.is_ok());
}

#[tokio::test]
async fn test_recover_pub_key() {
    let ctx = StateMachineTestContext::reset_and_lock().await;
    let client = ctx.client(ctx.canisters.oracular, ctx.admin_name());

    let signed_message = ethers_core::utils::hash_message("hello world");
    let rand_signature = ethereum_types::Signature::random();

    // let signature = ethers_core::types::Signature::

    let res = client
        .update::<(Principal,), Result<()>>("set_owner", (alice(),))
        .await
        .unwrap();

    assert!(res.is_ok());

    // Assert owner is set
    let res = client.query::<(), Principal>("owner", ()).await.unwrap();

    assert_eq!(res, alice());
}
