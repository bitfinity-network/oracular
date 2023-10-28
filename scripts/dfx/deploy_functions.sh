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

    # Which means the evm wasm file is either evm.wasm or evm_testnet.wasm

    dfx build --network=$NETWORK

    deploy_eth_rpc_canister "$NETWORK" "$INSTALL_MODE" "$OWNER"

    eth_rpc=$(dfx canister --network=$NETWORK id eth_rpc)

    deploy_oracular_canister "$NETWORK" "$INSTALL_MODE" "$OWNER" "$eth_rpc"

    oracular=$(dfx canister --network=$NETWORK id oracular)

    echo "Deployed oracular canister: $oracular"
    echo "Deployed eth_rpc canister: $eth_rpc"

}

deploy_oracular_canister() {
    NETWORK=$1
    INSTALL_MODE=$2
    OWNER=$3
    ETH_RPC=$4

    oracular_init="(record {
        owner=principal \"$OWNER\";
        ic_eth_rpc=opt principal \"$ETH_RPC\";
    })"

    echo "Deploying EVM canister with init args: $oracular_init"

    dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK --argument="$oracular_init" oracular
}

deploy_eth_rpc_canister() {
    NETWORK=$1
    INSTALL_MODE=$2
    OWNER=$3

    dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK eth_rpc --argument="()"
}
