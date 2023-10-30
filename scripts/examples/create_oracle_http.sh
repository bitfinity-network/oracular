#!/usr/bin/env sh
set -e

create_oracle_http() {
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
            contract = \"0x5d1fe823127eE6381D3b4752cF56B41373e198a2\";
            provider = record {
                chain_id = 355113;
                hostname = \"https://rich-queens-sell.loca.lt/?canisterId=bkyz2-fmaaa-aaaaa-qaaaq-cai\";
                credential_path = \"\";
            }
        }
    )"

    dfx canister call oracular create_oracle "$oracle_args"
}

create_oracle_http
