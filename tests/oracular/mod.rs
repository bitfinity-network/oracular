use core::panic;

use candid::{Decode, Encode, Principal};
use did::H160;
use ic_canister_client::CanisterClient;
use ic_exports::ic_kit::mock_principals::alice;

use ic_exports::ic_test_state_machine::WasmResult;
use oracular::canister::{EvmDestination, HttpOrigin, Origin};
use oracular::error::Result;
use oracular::eth_rpc::{InitProvider, ProviderView};

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

    let origin = Origin::Http(HttpOrigin {
        url: String::from("https://api.coinbase.com/v2/prices/BTC-ETH/spot"),
        json_path: String::from("data.amount"),
    });

    let destination = EvmDestination {
        contract: H160::from_hex_str("0x637F877db257ccba80B1fe06b0bEA039cd92C736").unwrap(),
        provider: InitProvider {
            chain_id: 355113,
            hostname: "https://127.0.0.1:8545".to_string(),
            credential_path: String::default(),
        },
    };

    let res = client
        .update::<(Origin, u64, EvmDestination), Result<()>>(
            "create_oracle",
            (origin, 1, destination),
        )
        .await
        .unwrap();
    let ic_eth_rpc = ctx.canisters.ic_eth_rpc;
    let providers = client
        .with_state_machine(move |env, _, caller| {
            let res = env
                .update_call(ic_eth_rpc, caller, "get_providers", (Encode!(&()).unwrap()))
                .unwrap();

            let bytes = match res {
                WasmResult::Reply(reply) => reply,
                WasmResult::Reject(e) => {
                    panic!("Error: {:?}", e);
                }
            };

            let decoded = Decode!(&bytes, Vec<ProviderView>).unwrap();

            decoded
        })
        .await;

    println!("providers : {:?}", providers);

    println!("res : {:?}", res);

    ctx.advance_time(std::time::Duration::from_secs(60 * 60 * 24 * 7))
        .await;

    // // Assert owner is set
    // let res = client.query::<(), Principal>("owner", ()).await.unwrap();

    // assert_eq!(res, alice());
}
