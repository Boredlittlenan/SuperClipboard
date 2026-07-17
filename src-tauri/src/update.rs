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

fn update_info_from_release_url(url: &reqwest::Url) -> Result<UpdateInfo, String> {
    let segments = url
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())
        .unwrap_or_default();
    let tag_name = match segments.as_slice() {
        [.., "releases", "tag", tag] if !tag.is_empty() => (*tag).to_string(),
        _ => {
            return Err(format!(
                "Latest release redirect did not contain a version tag: {url}"
            ))
        }
    };

    update_info(GithubRelease {
        tag_name,
        html_url: url.to_string(),
        name: String::new(),
        body: String::new(),
        published_at: String::new(),
        assets: Vec::new(),
    })
}

async fn fetch_release_api(client: &reqwest::Client) -> Result<GithubRelease, String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Update request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Update server returned an error: {error}"))?
        .json::<GithubRelease>()
        .await
        .map_err(|error| format!("Failed to parse update response: {error}"))
}

async fn fetch_release_page(client: &reqwest::Client) -> Result<UpdateInfo, String> {
    let url = format!("https://github.com/{GITHUB_REPO}/releases/latest");
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Release page request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Release page returned an error: {error}"))?;
    update_info_from_release_url(response.url())
}

#[tauri::command]
pub async fn check_update() -> Result<UpdateInfo, String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("SuperClipboard/{APP_VERSION}"))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|error| format!("Failed to create update client: {error}"))?;

    match fetch_release_api(&client).await {
        Ok(release) => update_info(release),
        Err(api_error) => fetch_release_page(&client)
            .await
            .map_err(|fallback_error| format!("{api_error}; fallback failed: {fallback_error}")),
    }
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

    #[test]
    fn parses_latest_release_redirect_without_api_metadata() {
        let url = reqwest::Url::parse(
            "https://github.com/Boredlittlenan/SuperClipboard/releases/tag/v3.6.0",
        )
        .unwrap();
        let info = update_info_from_release_url(&url).unwrap();

        assert_eq!(info.latest_version, "3.6.0");
        assert_eq!(info.download_url, url.as_str());
        assert!(info.has_update);
        assert!(info.release_notes.is_empty());
    }

    #[test]
    fn rejects_release_redirect_without_tag() {
        let url =
            reqwest::Url::parse("https://github.com/Boredlittlenan/SuperClipboard/releases/latest")
                .unwrap();
        assert!(update_info_from_release_url(&url).is_err());
    }
}
