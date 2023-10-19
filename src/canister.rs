use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use candid::{CandidType, Principal};
use eth_signer::sign_strategy::SigningStrategy;
use ic_canister::{init, query, Canister, PreUpdate};
use serde::{Deserialize, Serialize};

use crate::state::{Settings, State};

#[derive(Debug, Canister, Clone)]
pub struct Oracular {
    #[id]
    pub principal: Principal,
    pub state: Rc<RefCell<State>>,
}

impl PreUpdate for Oracular {}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct InitData {
    pub owner: Principal,
    pub evm: Principal,
    pub signing: SigningStrategy,
}

impl Oracular {
    #[init]
    pub fn init(&mut self, data: InitData) {
        let settings = Settings {
            owner: data.owner,
            evm: data.evm,
        };

        State::new(settings);

        // TODO: Set timers
    }

    pub fn state(&self) -> impl Deref<Target = State> + '_ {
        self.state.borrow()
    }

    pub fn state_mut(&mut self) -> impl DerefMut<Target = State> + '_ {
        self.state.borrow_mut()
    }

    #[query]
    pub fn owner(&self) -> Principal {
        todo!()
    }

    #[query]
    pub fn evm(&self) -> Principal {
        todo!()
    }
}
