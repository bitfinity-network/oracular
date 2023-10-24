use ic_canister::generate_idl;

use crate::canister::Oracular;

mod canister;
mod context;
mod contract;
mod error;
pub mod gen;
mod http;
mod memory;
mod processor;
mod state;

pub fn idl() -> String {
    use candid::Principal;
    use canister::InitData;
    use did::H160;

    let oracle_idl = Oracular::idl();

    candid::bindings::candid::compile(&oracle_idl.env.env, &Some(oracle_idl.actor))
}
