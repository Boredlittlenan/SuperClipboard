use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_REPO: &str = "Boredlittlenan/SuperClipboard";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub download_url: String,
    pub has_update: bool,
    pub release_name: String,
    pub release_notes: String,
    pub published_at: String,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    published_at: String,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

fn parse_version(value: &str) -> Result<Version, String> {
    let normalized = value
        .strip_prefix('v')
        .or_else(|| value.strip_prefix('V'))
        .unwrap_or(value);
    Version::parse(normalized).map_err(|error| format!("Invalid release version {value}: {error}"))
}

fn is_newer_version(latest: &str, current: &str) -> Result<bool, String> {
    Ok(parse_version(latest)? > parse_version(current)?)
}

fn installer_url(release: &GithubRelease) -> String {
    release
        .assets
        .iter()
        .find(|asset| asset.name.ends_with("_x64-setup.exe"))
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|asset| asset.name.ends_with(".exe"))
        })
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|asset| asset.name.ends_with(".msi"))
        })
        .map(|asset| asset.browser_download_url.clone())
        .unwrap_or_else(|| release.html_url.clone())
}

fn update_info(release: GithubRelease) -> Result<UpdateInfo, String> {
    let latest = release
        .tag_name
        .strip_prefix('v')
        .or_else(|| release.tag_name.strip_prefix('V'))
        .unwrap_or(&release.tag_name)
        .to_string();
    let download_url = installer_url(&release);
    let release_name = if release.name.trim().is_empty() {
        release.tag_name.clone()
    } else {
        release.name.clone()
    };
    let release_notes = release
        .body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(UpdateInfo {
        current_version: APP_VERSION.to_string(),
        latest_version: latest.clone(),
        download_url,
        has_update: is_newer_version(&latest, APP_VERSION)?,
        release_name,
        release_notes,
        published_at: release.published_at,
    })
}

#[tauri::command]
pub async fn check_update() -> Result<UpdateInfo, String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent(format!("SuperClipboard/{APP_VERSION}"))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|error| format!("Failed to create update client: {error}"))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Update request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Update server returned an error: {error}"))?;
    let release = response
        .json::<GithubRelease>()
        .await
        .map_err(|error| format!("Failed to parse update response: {error}"))?;

    update_info(release)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semantic_versions() {
        assert!(is_newer_version("3.10.0", "3.9.0").unwrap());
        assert!(is_newer_version("v3.2.1", "3.2.0").unwrap());
        assert!(!is_newer_version("3.2.0", "3.2.0").unwrap());
        assert!(!is_newer_version("3.2.0-beta.1", "3.2.0").unwrap());
    }

    #[test]
    fn prefers_windows_installer_asset() {
        let release = GithubRelease {
            tag_name: "v3.3.0".to_string(),
            html_url: "https://example.com/release".to_string(),
            name: String::new(),
            body: String::new(),
            published_at: String::new(),
            assets: vec![
                GithubAsset {
                    name: "SuperClipboard_3.3.0_x64_en-US.msi".to_string(),
                    browser_download_url: "https://example.com/app.msi".to_string(),
                },
                GithubAsset {
                    name: "SuperClipboard_3.3.0_x64-setup.exe".to_string(),
                    browser_download_url: "https://example.com/setup.exe".to_string(),
                },
            ],
        };

        assert_eq!(installer_url(&release), "https://example.com/setup.exe");
    }
}
