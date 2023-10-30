use crate::canister::Oracular;

pub mod canister;
mod context;
pub mod error;
mod http;
mod memory;
mod parser;
pub mod provider;
mod state;

pub mod constants;

pub fn idl() -> String {
    let oracle_idl = Oracular::idl();

    candid::bindings::candid::compile(&oracle_idl.env.env, &Some(oracle_idl.actor))
}
