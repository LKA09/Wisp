use anyhow::{Context, Result, bail};
use std::io::Read;

const REPO_OWNER: &str = "LKA09";
const REPO_NAME: &str = "Wisp";
const NPM_PACKAGE: &str = "@lka09/wisp";

pub fn run() -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");

    println!("Checking for updates...");
    println!("Current version: v{current}");

    let latest = fetch_latest_version()?;
    println!("Latest version:  v{latest}");

    if parse_semver(&latest) <= parse_semver(current) {
        println!("Already up to date.");
        return Ok(());
    }

    println!("Downloading v{latest}...");
    let asset = platform_asset()?;
    let data = download_asset(&asset, &latest)?;

    self_replace(&data)?;

    Ok(())
}

fn fetch_latest_version() -> Result<String> {
    let url = format!("https://registry.npmjs.org/{NPM_PACKAGE}/latest");
    let json: serde_json::Value = ureq::get(&url)
        .call()
        .context("Failed to reach npm registry")?
        .into_json()
        .context("Failed to parse npm registry response")?;
    let version = json["version"]
        .as_str()
        .context("No version field in npm response")?
        .to_string();
    Ok(version)
}

fn parse_semver(v: &str) -> (u32, u32, u32) {
    let mut parts = v.trim_start_matches('v').splitn(3, '.');
    let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

fn platform_asset() -> Result<String> {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        bail!("Unsupported OS for auto-update. Please update manually.");
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        bail!("Unsupported architecture for auto-update. Please update manually.");
    };

    let ext = if cfg!(target_os = "windows") { ".exe" } else { "" };
    Ok(format!("wisp-{os}-{arch}{ext}"))
}

fn download_asset(asset: &str, version: &str) -> Result<Vec<u8>> {
    let url = format!(
        "https://github.com/{REPO_OWNER}/{REPO_NAME}/releases/download/v{version}/{asset}"
    );
    println!("Downloading: {url}");

    let response = ureq::get(&url)
        .call()
        .context("Failed to download update from GitHub")?;

    let mut data = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut data)
        .context("Failed to read download")?;
    Ok(data)
}

#[cfg(unix)]
fn self_replace(data: &[u8]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let exe = std::env::current_exe().context("Cannot determine current executable path")?;
    let tmp = exe.with_extension("tmp");
    std::fs::write(&tmp, data).context("Failed to write new binary")?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&tmp, &exe).context("Failed to replace binary")?;
    println!("Updated successfully. Run `wisp` to use the new version.");
    Ok(())
}

#[cfg(windows)]
fn self_replace(data: &[u8]) -> Result<()> {
    // On Windows the running exe is locked; use a batch script to swap after exit.
    let exe = std::env::current_exe().context("Cannot determine current executable path")?;
    let dir = exe.parent().context("Cannot determine binary directory")?;
    let new_exe = dir.join("wisp_update.exe");
    let batch = dir.join("wisp_update.bat");

    std::fs::write(&new_exe, data).context("Failed to write new binary")?;

    let batch_content = format!(
        "@echo off\r\ntimeout /t 2 /nobreak >nul\r\nmove /y \"{new}\" \"{exe}\"\r\ndel \"%~f0\"\r\n",
        new = new_exe.display(),
        exe = exe.display(),
    );
    std::fs::write(&batch, batch_content).context("Failed to write updater script")?;

    std::process::Command::new("cmd")
        .args(["/c", &batch.to_string_lossy()])
        .spawn()
        .context("Failed to launch updater script")?;

    println!("Update downloaded. Wisp will finish updating after this process exits.");
    std::process::exit(0);
}

#[cfg(not(any(unix, windows)))]
fn self_replace(_data: &[u8]) -> Result<()> {
    bail!("Self-update is not supported on this platform. Please update manually.")
}
