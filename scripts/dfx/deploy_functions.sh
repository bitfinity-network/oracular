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
    LOG_SETTINGS=$4

    dfx build --network=$NETWORK

    deploy_oracular_canister "$NETWORK" "$INSTALL_MODE" "$OWNER" "$LOG_SETTINGS"

    oracular=$(dfx canister --network=$NETWORK id oracular)

    echo "Deployed oracular canister: $oracular"

}

deploy_oracular_canister() {
    NETWORK=$1
    INSTALL_MODE=$2
    OWNER=$3
    LOG_SETTINGS=$4

    oracular_init="(record {
        owner=principal \"$OWNER\";
        log_settings=$LOG_SETTINGS;
    })"

    echo "Deploying EVM canister with init args: $oracular_init"

    dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK --argument="$oracular_init" oracular
}
