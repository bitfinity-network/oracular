use crate::canister::Oracular;

pub mod canister;
mod context;
pub mod error;
pub mod eth_rpc;
mod http;
mod memory;
mod parser;
mod state;

pub fn idl() -> String {
    let oracle_idl = Oracular::idl();

    candid::bindings::candid::compile(&oracle_idl.env.env, &Some(oracle_idl.actor))
}
