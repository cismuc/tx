use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::path::Path;

#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(serde::Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

pub fn get_target_asset_name() -> Option<&'static str> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return Some("tx-macos-arm64");
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return Some("tx-macos-x86_64");
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return Some("tx-linux-x86_64");
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return Some("tx-linux-arm64");
    #[cfg(all(target_os = "linux", target_arch = "arm"))]
    return Some("tx-linux-armv7");
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return Some("tx-windows-x86_64.exe");
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    return Some("tx-windows-arm64.exe");
    #[allow(unreachable_code)]
    None
}

pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let s = v.strip_prefix('v').unwrap_or(v);
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.split('-').next()?.parse().ok()?;
    Some((major, minor, patch))
}

pub fn is_newer_version(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

pub fn check_for_update() -> Result<Option<(String, String, u64)>> {
    let target_asset = match get_target_asset_name() {
        Some(a) => a,
        None => return Ok(None),
    };

    let url = "https://api.github.com/repos/cismuc/tx/releases/latest";
    let mut res = ureq::get(url)
        .header("User-Agent", "tx-updater")
        .header("Accept", "application/vnd.github.v3+json")
        .call()
        .context("Failed to query GitHub release API")?;

    let release: GithubRelease = serde_json::from_reader(res.body_mut().as_reader())
        .context("Failed to parse release JSON")?;

    let latest_version = release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name);
    let current_version = env!("CARGO_PKG_VERSION");

    if is_newer_version(latest_version, current_version) {
        if let Some(asset) = release.assets.into_iter().find(|a| a.name == target_asset) {
            return Ok(Some((
                latest_version.to_string(),
                asset.browser_download_url,
                asset.size,
            )));
        }
    }

    Ok(None)
}

pub fn download_and_install<F, G>(
    download_url: &str,
    expected_size: u64,
    version: &str,
    mut on_progress: F,
    mut on_installing: G,
) -> Result<()>
where
    F: FnMut(u8, u64, u64),
    G: FnMut(),
{
    let mut res = ureq::get(download_url)
        .header("User-Agent", "tx-updater")
        .call()
        .context("Failed to request update binary download")?;

    let total_size = res
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(expected_size);

    let current_exe = std::env::current_exe().context("Failed to find current executable path")?;
    let current_dir = current_exe.parent().unwrap_or(Path::new("."));

    // Write to a temporary file on the same filesystem
    let temp_file = current_dir.join(format!(".tx-update-{}-{}", version, std::process::id()));

    let res_reader = res.body_mut().as_reader();
    let mut reader = res_reader;

    let mut file = std::fs::File::create(&temp_file).with_context(|| {
        format!(
            "Failed to create temporary file at {} (check write permissions)",
            temp_file.display()
        )
    })?;

    let mut buffer = [0u8; 65536];
    let mut downloaded: u64 = 0;
    let mut last_percent: u8 = 0;

    let write_result: Result<()> = (|| {
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            file.write_all(&buffer[..n])?;
            downloaded += n as u64;

            let percent = if total_size > 0 {
                ((downloaded as f64 / total_size as f64) * 100.0).min(100.0) as u8
            } else {
                0
            };

            if percent != last_percent {
                last_percent = percent;
                on_progress(percent, downloaded, total_size);
            }
        }
        file.flush()?;
        Ok(())
    })();

    if let Err(err) = write_result {
        let _ = std::fs::remove_file(&temp_file);
        return Err(err).context("Failed while writing update binary stream");
    }

    drop(file);
    on_installing();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        if let Err(e) = std::fs::set_permissions(&temp_file, perms) {
            let _ = std::fs::remove_file(&temp_file);
            return Err(e).context("Failed to set execute permissions on new binary");
        }
    }

    #[cfg(unix)]
    {
        if let Err(e) = std::fs::rename(&temp_file, &current_exe) {
            let _ = std::fs::remove_file(&temp_file);
            return Err(e).context("Failed to replace running executable with updated binary");
        }
    }

    #[cfg(windows)]
    {
        let backup_file = current_dir.join(format!(".tx-old-{}", std::process::id()));
        let _ = std::fs::remove_file(&backup_file);
        std::fs::rename(&current_exe, &backup_file).context("Failed to rename old executable")?;
        if let Err(e) = std::fs::rename(&temp_file, &current_exe) {
            let _ = std::fs::rename(&backup_file, &current_exe);
            let _ = std::fs::remove_file(&temp_file);
            return Err(e).context("Failed to install new executable on Windows");
        }
        let _ = std::fs::remove_file(&backup_file);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("1.0.0"), Some((1, 0, 0)));
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("v2.10.5-beta"), Some((2, 10, 5)));
        assert_eq!(parse_version("invalid"), None);
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("1.2.0", "1.1.0"));
        assert!(is_newer_version("v1.2.0", "1.1.0"));
        assert!(is_newer_version("2.0.0", "1.9.9"));
        assert!(is_newer_version("1.1.1", "1.1.0"));

        assert!(!is_newer_version("1.1.0", "1.1.0"));
        assert!(!is_newer_version("1.0.0", "1.1.0"));
        assert!(!is_newer_version("v1.0.0", "1.1.0"));
        assert!(!is_newer_version("invalid", "1.1.0"));
    }

    #[test]
    fn test_target_asset_name() {
        let asset = get_target_asset_name();
        assert!(asset.is_some(), "Asset should be defined for current platform");
        let name = asset.unwrap();
        assert!(name.starts_with("tx-"));
    }
}
