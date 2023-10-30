#!/bin/bash
set -e

WASM_DIR=.artifact

create() {
    # Create canisters
    NETWORK=$1

    dfx canister --network=$NETWORK create --with-cycles=600000000000 --all
}

deploy() {
    set -e

    NETWORK=$1
    INSTALL_MODE=$2
    OWNER=$3

    dfx build --network=$NETWORK

    deploy_oracular_canister "$NETWORK" "$INSTALL_MODE" "$OWNER"

    oracular=$(dfx canister --network=$NETWORK id oracular)

    dfx canister update-settings --add-controller $oracular eth_rpc

    echo "Deployed oracular canister: $oracular"
    echo "Deployed eth_rpc canister: $eth_rpc"

}

deploy_oracular_canister() {
    NETWORK=$1
    INSTALL_MODE=$2
    OWNER=$3

    oracular_init="(record {
        owner=principal \"$OWNER\";
    })"

    echo "Deploying EVM canister with init args: $oracular_init"

    dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK --argument="$oracular_init" oracular
}

create_oracle() {
    oracle_args="(
        \"0xfB0D14c07DA958bBB257346F49b2E9C9382c4888\",
        variant {
            Http = record {
            url = \"https://api.coinbase.com/v2/prices/BTC-ETH/spot\";
            json_path = \"data.amount\";
        }
        },
        10,
        record {
            contract = \"0xAda59057F9F53a48E0eA9C28D78aBBAD2C167B9D\";
            provider = record {
                chain_id = 355113;
                hostname = \"https://4fe7g-7iaaa-aaaak-aegcq-cai.raw.ic0.app\";
                credential_path = \"\";
            }
        }
    )"

    dfx canister call oracular create_oracle "$oracle_args"
}
