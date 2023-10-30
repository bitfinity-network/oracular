#!/usr/bin/env sh
set -e

create_oracle_evm() {
    oracle_args="(
        \"0xfB0D14c07DA958bBB257346F49b2E9C9382c4888\",
        variant {
            Evm = record {
                provider = record {
                    chain_id = 1;
                    hostname = \"https://eth-mainnet.alchemyapi.io/v2/demo\";

                };
                target_address = \"0xf4030086522a5beea4988f8ca5b36dbc97bee88c\";
                method = \"latestAnswer\";
            }
        },
        10,
        record {
            contract = \"0x5d1fe823127eE6381D3b4752cF56B41373e198a2\";
            provider = record {
                chain_id = 355113;
                hostname = \"https://clean-pillows-stare.loca.lt/?canisterId=bkyz2-fmaaa-aaaaa-qaaaq-cai\";
            }
        }
    )"

    dfx canister call oracular create_oracle "$oracle_args"
}

create_oracle_evm
