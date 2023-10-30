use candid::CandidType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum Source {
    Url(String),
    Provider(u64),
    Chain(u64),
    Service {
        hostname: String,
        chain_id: Option<u64>,
    },
}

#[derive(Debug, CandidType, Deserialize, Serialize, Clone)]
pub struct InitProvider {
    pub chain_id: u64,
    pub hostname: String,
    pub credential_path: String,
}
