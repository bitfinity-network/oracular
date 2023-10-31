use candid::Principal;
use did::H160;
use ic_canister_client::CanisterClient;
use ic_exports::ic_kit::mock_principals::alice;
use oracular::canister::{EvmDestination, HttpOrigin, Origin};
use oracular::error::Result;
use oracular::provider::Provider;
use oracular::state::oracle_storage::OracleMetadata;
use oracular::state::UpdateOracleMetadata;

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

    let user_address = H160::from_slice(&[5; 20]);

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
        .update::<(H160, Origin, u64, EvmDestination), Result<()>>(
            "create_oracle",
            (user_address.clone(), origin.clone(), 1, destination.clone()),
        )
        .await
        .unwrap();

    ctx.advance_time(std::time::Duration::from_secs(10)).await;

    assert!(res.is_ok());
    // get_user_oracles
    let res = client
        .query::<(H160,), Result<Vec<(H160, OracleMetadata)>>>(
            "get_user_oracles",
            (user_address.clone(),),
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(res.len(), 1);

    let oracle = res.get(0).unwrap();

    assert_eq!(oracle.0, destination.contract);
    assert_eq!(oracle.1.origin, origin);
    assert_eq!(oracle.1.evm, destination);
    assert_eq!(oracle.1.timer_interval, 1);
}

#[tokio::test]
async fn test_update_oracle() {
    let ctx = StateMachineTestContext::reset_and_lock().await;

    let client = ctx.client(ctx.canisters.oracular, ctx.admin_name());

    let user_address = H160::from_slice(&[5; 20]);

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
    client
        .update::<(H160, Origin, u64, EvmDestination), Result<()>>(
            "create_oracle",
            (user_address.clone(), origin.clone(), 1, destination.clone()),
        )
        .await
        .unwrap()
        .unwrap();

    ctx.advance_time(std::time::Duration::from_secs(10)).await;

    let new_origin = Origin::Http(HttpOrigin {
        url: String::from("https://example.com"),
        json_path: String::from("data"),
    });

    let update_metadata = UpdateOracleMetadata {
        origin: Some(new_origin.clone()),
        evm: None,
        timestamp: None,
    };

    client
        .update::<(H160, H160, UpdateOracleMetadata), Result<()>>(
            "update_oracle_metadata",
            (user_address.clone(), destination.contract, update_metadata),
        )
        .await
        .unwrap()
        .unwrap();

    ctx.advance_time(std::time::Duration::from_secs(10)).await;

    let res = client
        .query::<(H160,), Result<Vec<(H160, OracleMetadata)>>>(
            "get_user_oracles",
            (user_address.clone(),),
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(res.len(), 1);

    let oracle = res.get(0).unwrap();

    assert_eq!(oracle.1.origin, new_origin);
}

#[tokio::test]
async fn delete_oracle() {
    let ctx = StateMachineTestContext::reset_and_lock().await;

    let client = ctx.client(ctx.canisters.oracular, ctx.admin_name());

    let user_address = H160::from_slice(&[5; 20]);

    let origin = Origin::Http(HttpOrigin {
        url: String::from("https://api.coinbase.com/v2/prices/BTC-ETH/spot"),
        json_path: String::from("data.amount"),
    });

    let destination = EvmDestination {
        contract: H160::from_hex_str("0x637F877db257ccba80B1fe06b0bEA039cd92C736").unwrap(),
        provider: Provider {
            chain_id: 355113,
            hostname: "https://example.com".to_string(),
        },
    };

    client
        .update::<(H160, Origin, u64, EvmDestination), Result<()>>(
            "create_oracle",
            (user_address.clone(), origin.clone(), 1, destination.clone()),
        )
        .await
        .unwrap()
        .unwrap();

    ctx.advance_time(std::time::Duration::from_secs(10)).await;

    let res = client
        .query::<(H160,), Result<Vec<(H160, OracleMetadata)>>>(
            "get_user_oracles",
            (user_address.clone(),),
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(res.len(), 1);

    client
        .update::<(H160, H160), Result<()>>(
            "delete_oracle",
            (user_address.clone(), destination.contract),
        )
        .await
        .unwrap()
        .unwrap();

    let res = client
        .query::<(H160,), Result<Vec<(H160, OracleMetadata)>>>(
            "get_user_oracles",
            (user_address.clone(),),
        )
        .await
        .unwrap()
        .unwrap_err();

    assert_eq!(res, oracular::error::Error::UserNotFound); // If user not found, it means the oracle was deleted
}
