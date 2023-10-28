#!/bin/bash

set -e

args=("$@")
# Install mode
INSTALL_MODE=${args[0]:-"unset"}
# Network
NETWORK=${args[2]:-"ic"}
# Wallet
WALLET=${args[3]:-"4cfzs-sqaaa-aaaak-aegca-cai"}

source ./scripts/dfx/deploy_functions.sh

entry_point() {
    dfx identity use EVM_DEPLOYER
    dfx identity --network="$NETWORK" set-wallet "$WALLET"

    OWNER=$(dfx identity get-principal)

    if [ "$INSTALL_MODE" = "create" ]; then
        create "$NETWORK"
        INSTALL_MODE="install"
        deploy "$NETWORK" "$INSTALL_MODE" "$OWNER"

    elif [ "$INSTALL_MODE" = "upgrade" ] || [ "$INSTALL_MODE" = "reinstall" ]; then
        deploy "$NETWORK" "$INSTALL_MODE" "$OWNER"
    else
        echo "Command Not Found!"
        exit 1
    fi
}

entry_point
