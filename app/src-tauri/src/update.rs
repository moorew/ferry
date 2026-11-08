//! The update check (D-028): once a day, ask GitHub for the latest release and,
//! if it's newer than this build, offer this platform's installer. One anonymous
//! request to the public releases API; nothing about the user or their clips is
//! sent. Turned off with `general.check_updates = false`.

use std::time::{Duration, SystemTime};

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::state::AppState;

const LATEST: &str = "https://api.github.com/repos/moorew/tailcopy/releases/latest";
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);
const FIRST_AFTER: Duration = Duration::from_secs(60);
/// How often to see whether a day has passed. The day is measured on the wall
/// clock, because a sleeping timer stops counting while the computer is asleep.
const TICK: Duration = Duration::from_secs(60 * 60);

/// The installer name suffix for this platform, as the release workflow names it.
const ASSET: &str = if cfg!(windows) {
    "-windows-x64-setup.exe"
} else if cfg!(target_os = "macos") {
    "-macos-universal.dmg"
} else {
    "-ubuntu-amd64.deb"
};

#[derive(Clone, Debug)]
pub struct Available {
    pub version: String,
    /// The installer for this platform, or the release page if it has none.
    pub url: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

pub fn start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_AFTER).await;
        let mut last: Option<SystemTime> = None;
        loop {
            let due = last.is_none_or(|t| t.elapsed().map_or(true, |e| e >= EVERY));
            if due && app.state::<AppState>().config().general.check_updates {
                last = Some(SystemTime::now());
                if let Err(e) = check(&app).await {
                    tracing::info!("update check failed: {e}");
                }
            }
            tokio::time::sleep(TICK).await;
        }
    });
}

/// Asks GitHub now and offers what it finds. Returns the newer version, if any.
pub async fn check(app: &AppHandle) -> Result<Option<String>, reqwest::Error> {
    let found = latest().await?;
    let version = found.as_ref().map(|a| a.version.clone());
    offer(app, found);
    Ok(version)
}

fn offer(app: &AppHandle, found: Option<Available>) {
    let state = app.state::<AppState>();
    let fresh = {
        let mut slot = state.update.lock().unwrap();
        let fresh = match (&*slot, &found) {
            (Some(old), Some(new)) => old.version != new.version,
            (None, Some(_)) => true,
            _ => false,
        };
        *slot = found.clone();
        fresh
    };
    if let (true, Some(found)) = (fresh, found) {
        tracing::info!(version = %found.version, "a newer TailCopy is available");
        crate::picker::notify(
            app,
            &format!(
                "TailCopy {} is available. Open TailCopy to download it.",
                found.version
            ),
        );
        crate::tray::refresh(app);
        crate::state::status_changed(app);
    }
}

/// The latest published release, if it's newer than this build.
async fn latest() -> Result<Option<Available>, reqwest::Error> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("TailCopy/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()?;
    let release: Release = client
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(newer(release, env!("CARGO_PKG_VERSION")))
}

fn newer(release: Release, current: &str) -> Option<Available> {
    if release.draft || release.prerelease {
        return None;
    }
    let version = release.tag_name.trim_start_matches('v');
    let (Ok(latest), Ok(ours)) = (
        semver::Version::parse(version),
        semver::Version::parse(current),
    ) else {
        return None;
    };
    if latest <= ours {
        return None;
    }
    let url = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(ASSET))
        .map(|a| a.browser_download_url.clone())
        .filter(|u| u.starts_with("https://github.com/moorew/tailcopy/"))
        .unwrap_or(release.html_url);
    if !url.starts_with("https://github.com/moorew/tailcopy/") {
        return None;
    }
    Some(Available {
        version: latest.to_string(),
        url,
    })
}

/// Opens the download for the available update in the browser.
pub fn open(app: &AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let url = app.state::<AppState>().update.lock().unwrap().clone();
    match url {
        Some(a) => app
            .opener()
            .open_url(a.url, None::<&str>)
            .map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            html_url: format!("https://github.com/moorew/tailcopy/releases/tag/{tag}"),
            draft: false,
            prerelease: false,
            assets: vec![Asset {
                name: format!("TailCopy-{}{ASSET}", tag.trim_start_matches('v')),
                browser_download_url: format!(
                    "https://github.com/moorew/tailcopy/releases/download/{tag}/TailCopy{ASSET}"
                ),
            }],
        }
    }

    #[test]
    fn offers_only_newer_releases_for_this_platform() {
        assert!(newer(release("v0.2.1"), "0.2.1").is_none());
        assert!(newer(release("v0.2.0"), "0.2.1").is_none());
        let found = newer(release("v0.3.0"), "0.2.1").unwrap();
        assert_eq!(found.version, "0.3.0");
        assert!(found.url.ends_with(ASSET));
        let mut pre = release("v0.4.0");
        pre.prerelease = true;
        assert!(newer(pre, "0.2.1").is_none());
        let mut elsewhere = release("v0.5.0");
        elsewhere.html_url = "https://example.com/".into();
        elsewhere.assets.clear();
        assert!(newer(elsewhere, "0.2.1").is_none());
    }
}
