use ic_canister_client::CanisterClientError;
use ic_exports::ic_test_state_machine::CallError;
use ic_test_utils::Error;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TestError {
    #[error(transparent)]
    Candid(#[from] candid::Error),

    #[error(transparent)]
    CanisterClient(#[from] CanisterClientError),

    #[error(transparent)]
    TestUtils(#[from] Error),
}

pub type Result<T> = std::result::Result<T, TestError>;

impl From<CallError> for TestError {
    fn from(e: CallError) -> Self {
        e.into()
    }
}
