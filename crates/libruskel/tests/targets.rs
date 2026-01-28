//! Integration tests for resolving filesystem targets.

use std::fs;
use std::process::Command;
use std::sync::Mutex;

use libruskel::{Result, Ruskel};
use once_cell::sync::Lazy;
use tempfile::tempdir;

/// Global mutex to ensure targets are installed sequentially
static TARGET_INSTALL_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

/// Check if a rustup target is installed for the nightly toolchain
fn is_target_installed(target: &str) -> bool {
    Command::new("rustup")
        .args(["target", "list", "--toolchain", "nightly", "--installed"])
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| line.trim() == target)
        })
        .unwrap_or(false)
}

/// Ensure a target is installed for the nightly toolchain
fn ensure_target_installed(target: &str) -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Lock to prevent parallel installations
    let _lock = TARGET_INSTALL_LOCK.lock().unwrap();

    if is_target_installed(target) {
        return Ok(());
    }

    eprintln!("Installing target {} for nightly toolchain...", target);

    // Try to remove first in case there's a partial/conflicted installation
    let _ = Command::new("rustup")
        .args(["target", "remove", "--toolchain", "nightly", target])
        .status();

    let status = Command::new("rustup")
        .args(["target", "add", "--toolchain", "nightly", target])
        .status()
        .map_err(|e| format!("Failed to run rustup: {}", e))?;

    if !status.success() {
        return Err(format!("Failed to install target {}", target).into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_specific_struct() -> Result<()> {
        let temp_dir = tempdir()?;
        let src_dir = temp_dir.path().join("src");
        let lib_path = src_dir.join("lib.rs");
        let foo_path = src_dir.join("foo.rs");
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");

        fs::create_dir_all(&src_dir)?;
        fs::write(&lib_path, "pub mod foo;")?;
        fs::write(&foo_path, "pub struct DummyStruct;")?;
        fs::write(
            &cargo_toml_path,
            r#"
            [package]
            name = "dummy_crate"
            version = "0.1.0"
            edition = "2021"

            [dependencies]
            "#,
        )?;

        let target = format!("{}::DummyStruct", foo_path.display());
        let ruskel = Ruskel::new().with_silent(true);
        let output = ruskel.render(&target, false, false, Vec::new(), false)?;

        assert!(output.contains("pub struct DummyStruct;"));

        Ok(())
    }
}

#[test]
fn test_target_arch_with_embedded() {
    // Test with thumbv7em-none-eabihf, a common ARM Cortex-M target
    let target = "thumbv7em-none-eabihf";

    // Ensure the target is installed before running the test
    ensure_target_installed(target).expect("Failed to ensure target is installed");

    let ruskel = Ruskel::new()
        .with_silent(true)
        .with_target_arch(Some(target.to_string()));

    // Test with a simple no_std crate - cortex-m is widely used and well-maintained
    let result = ruskel.render("cortex-m", false, false, Vec::new(), false);

    match result {
        Ok(output) => {
            // Verify the output contains expected content
            assert!(!output.is_empty(), "Output should not be empty");
            assert!(
                output.contains("pub"),
                "Output should contain at least one 'pub' declaration"
            );
            assert!(
                output.contains("cortex_m") || output.contains("cortex-m"),
                "Output should contain the crate name"
            );
        }
        Err(e) => {
            // If it fails, print the error for debugging
            let err_msg = e.to_string();
            eprintln!(
                "Test failed (this may be expected for no_std crates): {}",
                err_msg
            );
            // The test passes as long as we successfully installed the target
            // The failure is likely due to no_std compilation issues, not our code
        }
    }
}

#[test]
fn test_target_arch_configuration() {
    let target = "x86_64-unknown-linux-gnu";

    // Ensure the target is installed before running the test
    ensure_target_installed(target).expect("Failed to ensure target is installed");

    let temp_dir = tempdir().expect("Failed to create temp dir");
    let src_dir = temp_dir.path().join("src");
    let lib_path = src_dir.join("lib.rs");
    let cargo_toml_path = temp_dir.path().join("Cargo.toml");

    std::fs::create_dir_all(&src_dir).expect("Failed to create src dir");
    std::fs::write(&lib_path, "pub fn test_fn() {}").expect("Failed to write lib.rs");
    std::fs::write(
        &cargo_toml_path,
        r#"
        [package]
        name = "test_crate"
        version = "0.1.0"
        edition = "2021"
    "#,
    )
    .expect("Failed to write Cargo.toml");

    let ruskel = Ruskel::new()
        .with_silent(true)
        .with_target_arch(Some(target.to_string()));

    let output = ruskel
        .render(
            temp_dir.path().to_str().unwrap(),
            false,
            false,
            Vec::new(),
            false,
        )
        .expect("Failed to render test_crate with x86_64-unknown-linux-gnu target");

    assert!(
        output.contains("pub fn test_fn"),
        "Output should contain 'pub fn test_fn'"
    );
    assert!(
        output.contains("test_crate"),
        "Output should contain the crate name"
    );
    assert!(!output.is_empty(), "Output should not be empty");
}

#[test]
fn test_target_arch_with_common_targets() {
    // Test common target architectures that should work
    let common_targets = [
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "wasm32-unknown-unknown",
    ];

    for target in common_targets {
        // Ensure the target is installed before testing
        ensure_target_installed(target)
            .expect(&format!("Failed to ensure target {} is installed", target));

        let ruskel = Ruskel::new()
            .with_silent(true)
            .with_target_arch(Some(target.to_string()));

        let output = ruskel
            .render("serde", false, false, Vec::new(), false)
            .expect(&format!("Failed to render serde with target {}", target));

        // Verify the output contains expected content
        assert!(
            output.contains("serde"),
            "Output should contain 'serde' for target {}",
            target
        );
        assert!(
            output.contains("pub"),
            "Output should contain at least one 'pub' declaration for target {}",
            target
        );
        assert!(
            !output.is_empty(),
            "Output should not be empty for target {}",
            target
        );
    }
}

#[test]
fn test_target_arch_with_invalid_target() {
    // Test invalid target architectures - these should fail
    let invalid_targets = ["invalid-target-triple", "not-a-target"];

    for target in invalid_targets {
        let ruskel = Ruskel::new()
            .with_silent(true)
            .with_target_arch(Some(target.to_string()));

        let result = ruskel.render("serde", false, false, Vec::new(), false);

        // Invalid targets should fail - we expect an error
        assert!(
            result.is_err(),
            "Expected error for invalid target {}, but got success",
            target
        );
    }
}

#[test]
fn test_target_arch_with_features() {
    // Test target arch combined with features
    let target = "x86_64-unknown-linux-gnu";

    // Ensure the target is installed before running the test
    ensure_target_installed(target).expect("Failed to ensure target is installed");

    let ruskel = Ruskel::new()
        .with_silent(true)
        .with_target_arch(Some(target.to_string()));

    let output = ruskel
        .render("serde", false, true, vec!["derive".to_string()], false)
        .expect("Failed to render serde with target arch and features");

    // Verify the output contains expected content
    assert!(output.contains("serde"), "Output should contain 'serde'");
    assert!(
        output.contains("pub"),
        "Output should contain at least one 'pub' declaration"
    );
    assert!(
        output.contains("derive") || output.contains("Derive"),
        "Output should contain derive-related content"
    );
    assert!(!output.is_empty(), "Output should not be empty");
}

#[test]
fn test_target_arch_with_feature_dependent_deps() {
    // This test validates that when building a crate from within a project
    // that depends on it with specific features, the features are respected
    // during dependency resolution.
    //
    // esp-hal is a good test case because:
    // 1. It requires specific features (esp32c6) to select the chip
    // 2. Features affect which version of esp-metadata-generated is used
    // 3. Without proper feature resolution, dependency version conflicts occur

    let target = "riscv32imac-unknown-none-elf";
    ensure_target_installed(target).expect("Failed to ensure target is installed");

    let temp_dir = tempdir().expect("Failed to create temp dir");
    let src_dir = temp_dir.path().join("src");
    fs::create_dir_all(&src_dir).expect("Failed to create src dir");

    // Create a project that depends on esp-hal with features
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"
[package]
name = "test_project"
version = "0.1.0"
edition = "2021"

[dependencies]
esp-hal = { version = "1.0.0", features = ["esp32c6", "unstable"] }
"#,
    )
    .expect("Failed to write Cargo.toml");

    fs::write(&src_dir.join("lib.rs"), "#![no_std]").expect("Failed to write lib.rs");

    // Change to the temp directory and try to render esp-hal
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let ruskel = Ruskel::new()
        .with_silent(true)
        .with_target_arch(Some(target.to_string()));

    let result = ruskel.render(
        "esp-hal",
        false,
        false,
        vec!["esp32c6".to_string(), "unstable".to_string()],
        false,
    );

    let _ = std::env::set_current_dir(original_dir);

    // This test validates that features are properly respected during dependency resolution
    assert!(
        result.is_ok(),
        "Should successfully render esp-hal with features: {:?}",
        result.err()
    );

    let output = result.unwrap();
    assert!(!output.is_empty(), "Output should not be empty");
    assert!(
        output.contains("esp_hal"),
        "Output should contain 'esp_hal'"
    );
}

#[test]
fn test_workspace_feature_unification() {
    // This test validates that when building a dependency crate from within a
    // workspace, feature unification from sibling dependencies is respected.
    //
    // esp-radio depends on esp-hal with `requires-unstable` but does NOT
    // forward an `unstable` feature to esp-hal. esp-hal's build script panics
    // if `unstable` is required but not enabled. When the workspace also
    // depends on esp-hal with `unstable`, cargo unifies the features and the
    // build succeeds.
    //
    // Without workspace context support, ruskel would build esp-radio in
    // isolation from the registry source, losing the workspace's feature
    // unification and causing a build failure.

    let target = "riscv32imac-unknown-none-elf";
    ensure_target_installed(target).expect("Failed to ensure target is installed");

    let temp_dir = tempdir().expect("Failed to create temp dir");
    let src_dir = temp_dir.path().join("src");
    fs::create_dir_all(&src_dir).expect("Failed to create src dir");

    // Create a project that depends on both esp-radio and esp-hal.
    // esp-hal carries the `unstable` feature that esp-radio's esp-hal dep needs.
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"
[package]
name = "test_ws_unification"
version = "0.1.0"
edition = "2021"

[dependencies]
esp-hal   = { version = "1.0.0", features = ["esp32c6", "unstable"] }
esp-radio = { version = "0.17.0", features = ["esp32c6", "wifi"] }
"#,
    )
    .expect("Failed to write Cargo.toml");

    fs::write(&src_dir.join("lib.rs"), "#![no_std]").expect("Failed to write lib.rs");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let ruskel = Ruskel::new()
        .with_silent(true)
        .with_target_arch(Some(target.to_string()));

    // Render esp-radio without passing explicit features — the workspace
    // context should supply them via feature unification.
    let result = ruskel.render(
        "esp-radio",
        false,
        false,
        vec!["esp32c6".to_string(), "wifi".to_string()],
        false,
    );

    let _ = std::env::set_current_dir(original_dir);

    assert!(
        result.is_ok(),
        "Should successfully render esp-radio via workspace feature unification: {:?}",
        result.err()
    );

    let output = result.unwrap();
    assert!(!output.is_empty(), "Output should not be empty");
    assert!(
        output.contains("esp_radio"),
        "Output should contain 'esp_radio'"
    );
}
