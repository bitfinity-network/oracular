//! Build the contract and generate the solidity bindings.
//!
use std::env;
use std::path::PathBuf;

use ethers::solc::{Project, ProjectPathsConfig};

fn compile(
    root: &PathBuf,
    source: &PathBuf,
    build_info: &PathBuf,
    target: &PathBuf,
) -> anyhow::Result<()> {
    let build_path_config = ProjectPathsConfig::builder()
        .sources(source)
        .artifacts(target)
        .build_infos(build_info)
        .root(root)
        .build()?;

    let project = Project::builder().paths(build_path_config).build()?;
    project.rerun_if_sources_changed();
    let compiled = project.compile()?;

    assert!(
        !compiled.has_compiler_errors(),
        "Compiling PriceFeed smart contracts failed: {:?}.",
        compiled.output().errors
    );

    Ok(())
}

fn compile_pf_smart_contracts() -> anyhow::Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("contracts");
    let target = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let build_info = root.join("contracts").join("build-info");

    // compile interfaces
    compile(&root, &root.join("interfaces"), &build_info, &target)?;

    // compile PriceFeed
    compile(
        &root,
        &root.join("core").join("PriceFeed.sol"),
        &build_info,
        &target,
    )?;

    // Compile Ownable
    compile(&root, &root.join("Ownable.sol"), &build_info, &target)?;

    Ok(())
}

fn main() {
    compile_pf_smart_contracts().expect("Compiling smart contracts failed.");
}
