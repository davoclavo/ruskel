use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

use crate::error::{Result, RuskelError, nightly_install_error};

/// Locate the nightly toolchain sysroot path.
pub fn nightly_sysroot(target_arch: Option<&str>) -> Result<PathBuf> {
    let mut args = vec!["+nightly", "--print", "sysroot"];
    let target_owned;
    if let Some(target) = target_arch {
        args.push("--target");
        target_owned = target.to_string();
        args.push(&target_owned);
    }

    let output = Command::new("rustc")
        .args(&args)
        .output()
        .map_err(|e| RuskelError::Generate(format!("Failed to get sysroot: {e}")))?;

    if !output.status.success() {
        return Err(RuskelError::Generate(nightly_install_error(
            "Failed to get nightly sysroot",
            target_arch,
        )));
    }

    let sysroot = String::from_utf8(output.stdout)
        .map_err(|e| RuskelError::Generate(format!("Invalid UTF-8 in sysroot path: {e}")))?
        .trim()
        .to_string();

    Ok(PathBuf::from(sysroot))
}

/// Ensure the nightly toolchain exists and report whether the `rust-docs-json` component is installed.
pub fn ensure_nightly_with_docs(target_arch: Option<&str>) -> Result<bool> {
    let output = Command::new("rustup")
        .args(["run", "nightly", "rustc", "--version"])
        .stderr(Stdio::null())
        .output()
        .map_err(|e| RuskelError::Generate(format!("Failed to run rustup: {e}")))?;

    if !output.status.success() {
        return Err(RuskelError::Generate(nightly_install_error(
            "ruskel requires the nightly toolchain to be installed",
            target_arch,
        )));
    }

    let components_output = Command::new("rustup")
        .args(["component", "list", "--toolchain", "nightly"])
        .stderr(Stdio::null())
        .output()
        .map_err(|e| RuskelError::Generate(format!("Failed to check nightly components: {e}")))?;

    if !components_output.status.success() {
        return Ok(false);
    }

    let has_rust_docs_json = String::from_utf8_lossy(&components_output.stdout)
        .lines()
        .any(|line| line.starts_with("rust-docs-json") && line.contains("(installed)"));

    Ok(has_rust_docs_json)
}
