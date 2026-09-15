//! Release checks. The app never self-updates; it only points the user at a
//! newer release page, and only trusts that page when the release carries a
//! valid minisign signature from the maintainer key embedded below. HTTP runs
//! on blocking workers like all I/O here.

use super::*;

pub(crate) const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/acramatte/foglio/releases/latest";

/// Minisign public key of the release signing key. The matching private key
/// lives only in the release workflow secret; assets this app cannot verify
/// are reported as unsigned rather than silently trusted.
const RELEASE_PUBLIC_KEY: &str = "RWTT27n1HSzADR0Sylcq5vOHoO1xUcbnU+VDibwyJpwXsMGmeFAGaWPL";

const MANIFEST_ASSET: &str = "release.txt";
const SIGNATURE_ASSET: &str = "release.txt.minisig";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SignatureState {
    /// The release carried a signature that verifies against the release key.
    Verified,
    /// No signature assets were published (legacy releases predating signing).
    Unsigned,
    /// Signature assets exist but fail verification: never link this release.
    Tampered,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
    pub signature: SignatureState,
}

/// The tauri command wrapper lives in commands.rs; this is its blocking body.
pub fn latest_release(url: &str, current: &str) -> Result<Option<UpdateInfo>> {
    let body = fetch(url)?;
    Ok(release_from_json(&body, current))
}

fn fetch(url: &str) -> Result<String> {
    let response = ureq::get(url)
        .set("User-Agent", "foglio-desktop")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(err)?;
    response.into_string().map_err(err)
}

fn asset_url(body: &serde_json::Value, name: &str) -> Option<String> {
    body["assets"].as_array()?.iter().find_map(|asset| {
        if asset["name"].as_str()? != name {
            return None;
        }
        asset["browser_download_url"].as_str().map(str::to_string)
    })
}

fn verify_release_signature(manifest: &str, signature: &str) -> bool {
    use minisign_verify::{PublicKey, Signature};
    let Ok(public_key) = PublicKey::from_base64(RELEASE_PUBLIC_KEY) else {
        return false;
    };
    let Ok(signature) = Signature::decode(signature) else {
        return false;
    };
    public_key
        .verify(manifest.as_bytes(), &signature, false)
        .is_ok()
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
    if !newer_version(version, current)? {
        return None;
    }
    let signature = match (
        asset_url(&release, MANIFEST_ASSET),
        asset_url(&release, SIGNATURE_ASSET),
    ) {
        (Some(manifest_url), Some(signature_url)) => {
            let verified = (|| {
                let manifest = fetch(&manifest_url).ok()?;
                let signature = fetch(&signature_url).ok()?;
                Some(verify_release_signature(&manifest, &signature))
            })()
            .unwrap_or(false);
            if verified {
                SignatureState::Verified
            } else {
                SignatureState::Tampered
            }
        }
        _ => SignatureState::Unsigned,
    };
    Some(UpdateInfo {
        version: version.into(),
        url: url.into(),
        signature,
    })
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

    // Test-only minisign key pair. The public half signs the fixtures below;
    // the private half exists to produce those fixtures and must never be
    // mistaken for the release key, whose public half RELEASE_PUBLIC_KEY holds.
    const TEST_PUBLIC_KEY: &str = "RWT7HHo9QQLZaqz3YPuwDsCbx9FEDbX62A4EbY3axnm2y8LwGgeFmzWp";
    const TEST_SIGNATURE: &str = "untrusted comment: \nRUT7HHo9QQLZajXArKJJytC3k4glFY9F5prRPMoa+0FF+VPQop8R3bIreLG4b+/d4d6uHLlqn0VmlOT7vzLMMY8whtGiNGBI9gY=\ntrusted comment: timestamp:1789461107\nd7jfKt24LHagThPUwTdJ1GLtsBLFBtRnmaUT1LxNcxWRz9GunFEQiG/Jj3fWRKaAq/MgNRT+OSbe1VGbDu8xAQ==\n";

    fn release_json(tag: &str, draft: bool, prerelease: bool, assets: &[(&str, &str)]) -> String {
        let assets: Vec<_> = assets
            .iter()
            .map(|(name, url)| {
                serde_json::json!({
                    "name": name,
                    "browser_download_url": url,
                })
            })
            .collect();
        serde_json::json!({
            "tag_name": tag,
            "html_url": "https://github.com/acramatte/foglio/releases/tag/v9.9.9",
            "draft": draft,
            "prerelease": prerelease,
            "assets": assets,
        })
        .to_string()
    }

    fn release(tag: &str, assets: &[(&str, &str)]) -> String {
        release_json(tag, false, false, assets)
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
    fn unsigned_newer_release_is_reported_without_signature() {
        let update = release_from_json(&release("v0.3.0", &[]), "0.2.1").unwrap();
        assert_eq!(update.signature, SignatureState::Unsigned);
        assert_eq!(update.version, "0.3.0");
        assert_eq!(
            update.url,
            "https://github.com/acramatte/foglio/releases/tag/v9.9.9"
        );
    }

    #[test]
    fn draft_prerelease_same_older_or_malformed_payloads_are_silent() {
        assert!(release_from_json(&release("v0.2.1", &[]), "0.2.1").is_none());
        assert!(release_from_json(&release("v0.2.0", &[]), "0.2.1").is_none());
        assert!(release_from_json(&release_json("v0.3.0", true, false, &[]), "0.2.1").is_none());
        assert!(release_from_json(&release_json("v0.3.0", false, true, &[]), "0.2.1").is_none());
        assert!(release_from_json(&release("0.3.0-rc1", &[]), "0.2.1").is_none());
        assert!(release_from_json("not json", "0.2.1").is_none());
        assert!(release_from_json("{}", "0.2.1").is_none());
    }

    #[test]
    fn asset_urls_are_matched_by_name_only() {
        let body: serde_json::Value = serde_json::from_str(&release(
            "v0.3.0",
            &[
                ("foglio_0.3.0_amd64.deb", "https://x/deb"),
                ("release.txt", "https://x/manifest"),
            ],
        ))
        .unwrap();
        assert_eq!(
            asset_url(&body, "release.txt").as_deref(),
            Some("https://x/manifest")
        );
        assert_eq!(asset_url(&body, "release.txt.minisig"), None);
    }

    #[test]
    fn test_signature_verifies_over_its_manifest() {
        assert!(verify_release_signature_with(
            TEST_PUBLIC_KEY,
            "v0.3.0\n",
            TEST_SIGNATURE
        ));
    }

    #[test]
    fn tampered_manifest_or_signature_fails() {
        assert!(!verify_release_signature_with(
            TEST_PUBLIC_KEY,
            "v0.4.0\n",
            TEST_SIGNATURE
        ));
        assert!(!verify_release_signature_with(
            TEST_PUBLIC_KEY,
            "v0.3.0",
            TEST_SIGNATURE
        ));
        assert!(!verify_release_signature_with(
            TEST_PUBLIC_KEY,
            "v0.3.0\n",
            "untrusted comment: \nRUT7HHo9QQLZajXArKJJytC3k4glFY9F5prRPMoa+0FF+VPQop8R3bIreLG4b+/d4d6uHLlqn0VmlOT7vzLMMY8whtGiNGBI9gY=\n"
        ));
        assert!(!verify_release_signature_with(
            "RWTnotakey",
            "v0.3.0\n",
            TEST_SIGNATURE
        ));
        assert!(!verify_release_signature_with(
            TEST_PUBLIC_KEY,
            "v0.3.0\n",
            "not a signature"
        ));
    }

    fn verify_release_signature_with(public_key: &str, manifest: &str, signature: &str) -> bool {
        use minisign_verify::{PublicKey, Signature};
        let Ok(public_key) = PublicKey::from_base64(public_key) else {
            return false;
        };
        let Ok(signature) = Signature::decode(signature) else {
            return false;
        };
        public_key
            .verify(manifest.as_bytes(), &signature, false)
            .is_ok()
    }
}
