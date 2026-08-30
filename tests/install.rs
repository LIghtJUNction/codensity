#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture {
    path: PathBuf,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!(
            "codensity-install-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn install_script_verifies_the_binary_in_a_custom_install_root()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let fake_bin = fixture.path.join("fake-bin");
    let install_root = fixture.path.join("install-root");
    fs::create_dir(&fake_bin)?;
    fs::create_dir(&install_root)?;
    fs::create_dir(install_root.join("bin"))?;
    symlink("/bin/true", fake_bin.join("cargo"))?;

    let installed_binary = install_root.join("bin/codensity");
    fs::write(
        &installed_binary,
        b"#!/bin/sh\nprintf 'custom-root-codensity\\n'\n",
    )?;
    let mut permissions = fs::metadata(&installed_binary)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&installed_binary, permissions)?;

    let output = Command::new("/bin/bash")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("install.sh"))
        .env("PATH", &fake_bin)
        .env("CARGO_INSTALL_ROOT", &install_root)
        .env_remove("CODENSITY_VERSION")
        .output()?;

    assert!(
        output.status.success()
            && String::from_utf8_lossy(&output.stdout).trim() == "custom-root-codensity",
        "status: {:?}, stdout: {}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(())
}
