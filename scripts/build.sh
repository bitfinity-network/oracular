#!/usr/bin/env sh
set -e
export RUST_BACKTRACE=full

# Configuration variables
WASM_DIR=".artifact"

# Initial setup
initialize_env() {

    if [ ! -f "./Cargo.toml" ]; then
        echo "Expecting to run from the cargo root directory, current directory is: $(pwd)"
        exit 42
    fi

    if [ "$CI" != "true" ]; then
        script_dir=$(dirname $0)
        project_dir=$(realpath "${script_dir}/..")

        echo "Project dir: \"$project_dir\""
        cd "$project_dir"

        rm -rf "$WASM_DIR"
        mkdir -p "$WASM_DIR"
    fi
}

# Function to build canisters
build_canister() {
    local package_name="$1"
    local features="$2"
    local wasm_dir="$3"
    local output_wasm="$4"
    local did_file_name="${5:-$package_name}"

    echo "Building $package_name Canister"

    cargo run -p "$package_name" --features "$features" >"$wasm_dir/$did_file_name.did"

    cargo build --target wasm32-unknown-unknown --release --package "$package_name" --features "$features"
    ic-wasm "target/wasm32-unknown-unknown/release/$package_name.wasm" -o "$wasm_dir/$output_wasm" shrink
    gzip -k "$wasm_dir/$output_wasm" --force
}

main() {
    # initialize_env

    echo "Building WASM modules"
    build_canister "oracular" "export-api" "$WASM_DIR" "oracular.wasm" "oracular"

}

main "$@"
