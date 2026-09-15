//! Release checks. The app never self-updates; it only points the user at a
//! newer signed release page. HTTP runs on blocking workers like all I/O here.

use super::*;

pub(crate) const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/acramatte/foglio/releases/latest";

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
}

/// The tauri command wrapper lives in commands.rs; this is its blocking body.
pub fn latest_release(url: &str, current: &str) -> Result<Option<UpdateInfo>> {
    let response = ureq::get(url)
        .set("User-Agent", "foglio-desktop")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(err)?;
    let body = response.into_string().map_err(err)?;
    Ok(release_from_json(&body, current))
}

/// Returns the update when the release tag is strictly newer than `current`.
/// Anything malformed (bad JSON, unparseable tags) yields None: an update
/// notice must never be wrong, and absence of data means silence.
fn release_from_json(body: &str, current: &str) -> Option<UpdateInfo> {
    let release: serde_json::Value = serde_json::from_str(body).ok()?;
    if release["draft"].as_bool().unwrap_or(true) || release["prerelease"].as_bool().unwrap_or(true)
    {
        return None;
    }
    let tag = release["tag_name"].as_str()?;
    let url = release["html_url"].as_str()?;
    let version = tag.strip_prefix('v')?;
    if newer_version(version, current)? {
        Some(UpdateInfo {
            version: version.into(),
            url: url.into(),
        })
    } else {
        None
    }
}

/// Strict numeric x.y.z comparison. Returns None when either side is not a
/// plain three-part numeric version rather than guessing an ordering.
fn newer_version(candidate: &str, current: &str) -> Option<bool> {
    fn parts(version: &str) -> Option<[u64; 3]> {
        let (major, rest) = version.split_once('.')?;
        let (minor, patch) = rest.split_once('.')?;
        if patch.contains('.') {
            return None;
        }
        Some([
            major.parse().ok()?,
            minor.parse().ok()?,
            patch.parse().ok()?,
        ])
    }
    let candidate = parts(candidate)?;
    let current = parts(current)?;
    Some(candidate > current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_json(tag: &str) -> String {
        serde_json::json!({
            "tag_name": tag,
            "html_url": "https://github.com/acramatte/foglio/releases/tag/v9.9.9",
            "draft": false,
            "prerelease": false,
        })
        .to_string()
    }

    #[test]
    fn newer_patch_minor_and_major_versions_are_reported() {
        assert!(newer_version("0.2.2", "0.2.1").unwrap());
        assert!(newer_version("0.3.0", "0.2.1").unwrap());
        assert!(newer_version("1.0.0", "0.2.1").unwrap());
        assert!(!newer_version("0.2.1", "0.2.1").unwrap());
        assert!(!newer_version("0.2.0", "0.2.1").unwrap());
        assert!(!newer_version("0.1.9", "0.2.1").unwrap());
    }

    #[test]
    fn malformed_versions_are_never_compared() {
        assert!(newer_version("0.2", "0.2.1").is_none());
        assert!(newer_version("0.2.1-beta", "0.2.1").is_none());
        assert!(newer_version("x.y.z", "0.2.1").is_none());
        assert!(newer_version("0.2.1.1", "0.2.1").is_none());
        assert!(newer_version("0.2.1", "").is_none());
    }

    #[test]
    fn newer_release_is_parsed_from_github_payload() {
        let update = release_from_json(&release_json("v0.3.0"), "0.2.1").unwrap();
        assert_eq!(
            update,
            UpdateInfo {
                version: "0.3.0".into(),
                url: "https://github.com/acramatte/foglio/releases/tag/v9.9.9".into(),
            }
        );
    }

    #[test]
    fn same_older_draft_prerelease_or_malformed_payloads_are_silent() {
        assert!(release_from_json(&release_json("v0.2.1"), "0.2.1").is_none());
        assert!(release_from_json(&release_json("v0.2.0"), "0.2.1").is_none());
        let mut draft = release_json("v0.3.0");
        draft = draft.replace("\"draft\":false", "\"draft\":true");
        assert!(release_from_json(&draft, "0.2.1").is_none());
        assert!(release_from_json(&release_json("0.3.0-rc1"), "0.2.1").is_none());
        assert!(release_from_json("not json", "0.2.1").is_none());
        assert!(release_from_json("{}", "0.2.1").is_none());
    }
}
