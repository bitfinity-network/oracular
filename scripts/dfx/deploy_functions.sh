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

    echo "Deployed oracular canister: $oracular"

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

# create_oracle() {
#     oracle_args="(
#         \"0xfB0D14c07DA958bBB257346F49b2E9C9382c4888\",
#         variant {
#             Http = record {
#             url = \"https://api.coinbase.com/v2/prices/BTC-ETH/spot\";
#             json_path = \"data.amount\";
#         }
#         },
#         10,
#         record {
#             contract = \"0x5d1fe823127eE6381D3b4752cF56B41373e198a2\";
#             provider = record {
#                 chain_id = 355113;
#                 hostname = \"https://rich-queens-sell.loca.lt/?canisterId=bkyz2-fmaaa-aaaaa-qaaaq-cai\";
#                 credential_path = \"\";
#             }
#         }
#     )"

#     dfx canister call oracular create_oracle "$oracle_args"
# }
