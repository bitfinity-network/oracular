pub mod error;
pub mod wasm;
use std::path::{Path, PathBuf};

use ic_exports::ic_test_state_machine::get_ic_test_state_machine_client_path;

pub fn get_state_machine_path() -> String {
    get_workspace_root_dir()
        .join("target")
        .into_os_string()
        .into_string()
        .unwrap()
}

pub async fn get_state_machine_bin_path() -> String {
    let path = get_state_machine_path();
    tokio::task::spawn_blocking(move || get_ic_test_state_machine_client_path(&path))
        .await
        .unwrap()
}

/// Returns the Path to the workspace root dir
pub fn get_workspace_root_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}
