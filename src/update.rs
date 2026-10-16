//! Self-update from GitHub releases.
//!
//! The check and the download run on a background thread. The UI only ever
//! sees a plain value, so a slow or missing network never stalls a frame.
//!
//! Release assets are plain executables named after the Rust target triple, so
//! the updater never has to unpack an archive.

use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

/// Where releases live. Override at build time with `CAMVIEWER_REPOSITORY`.
pub const REPOSITORY: &str = match option_env!("CAMVIEWER_REPOSITORY") {
    Some(repository) => repository,
    None => "peyloride/camviewer",
};

/// The running version, taken from `Cargo.toml`.
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The asset name this build looks for, named after the Rust target triple.
pub fn asset_name() -> String {
    let os = match std::env::consts::OS {
        "macos" => "apple-darwin",
        "windows" => "pc-windows-msvc",
        _ => "unknown-linux-gnu",
    };
    let extension = if cfg!(windows) { ".exe" } else { "" };

    format!("camviewer-{}-{os}{extension}", std::env::consts::ARCH)
}

/// A release that is newer than the running build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub size: u64,
}

/// Asks GitHub for the newest release. `Ok(None)` means "already current".
pub fn check(current: &str) -> Result<Option<Release>, String> {
    let endpoint = format!("https://api.github.com/repos/{REPOSITORY}/releases/latest");
    let body = ureq::get(&endpoint)
        .header("Accept", "application/vnd.github+json")
        .header(
            "User-Agent",
            concat!("camviewer/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|err| format!("cannot reach GitHub: {err}"))?
        .body_mut()
        .read_to_string()
        .map_err(|err| format!("cannot read the reply from GitHub: {err}"))?;

    let release: serde_json::Value = serde_json::from_str(&body)
        .map_err(|err| format!("unexpected reply from GitHub: {err}"))?;

    release_from_json(&release, current, &asset_name())
}

/// Reads one release from GitHub's reply.
///
/// `wanted` is the asset this machine needs, which `check` fills in from
/// [`asset_name`]. It is a parameter so this can be tested anywhere.
pub fn release_from_json(
    release: &serde_json::Value,
    current: &str,
    wanted: &str,
) -> Result<Option<Release>, String> {
    let tag = release["tag_name"]
        .as_str()
        .ok_or("the newest release has no tag")?;
    let version = tag.trim_start_matches('v').to_string();

    if !is_newer(&version, current) {
        return Ok(None);
    }

    let asset = release["assets"]
        .as_array()
        .ok_or("the newest release lists no files")?
        .iter()
        .find(|asset| asset["name"].as_str() == Some(wanted))
        .ok_or_else(|| format!("release {tag} has no build for this machine ({wanted})"))?;

    Ok(Some(Release {
        version,
        notes: release["body"].as_str().unwrap_or_default().to_string(),
        url: asset["browser_download_url"]
            .as_str()
            .ok_or("that build has no download address")?
            .to_string(),
        size: asset["size"].as_u64().unwrap_or(0),
    }))
}

/// Downloads the new build and puts it in place of the running one.
///
/// The caller restarts the app afterwards; this process keeps running the old
/// image until then.
pub fn install(release: &Release) -> Result<(), String> {
    let staged = staging_path(&release.version);

    let mut response = ureq::get(&release.url)
        .header(
            "User-Agent",
            concat!("camviewer/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|err| format!("cannot download the update: {err}"))?;

    {
        let mut file = fs::File::create(&staged)
            .map_err(|err| format!("cannot write to {}: {err}", staged.display()))?;
        std::io::copy(&mut response.body_mut().as_reader(), &mut file)
            .map_err(|err| format!("the download stopped early: {err}"))?;
        file.flush()
            .map_err(|err| format!("cannot finish writing the update: {err}"))?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))
            .map_err(|err| format!("cannot mark the update as runnable: {err}"))?;
    }

    self_replace::self_replace(&staged)
        .map_err(|err| format!("cannot put the update in place: {err}"))?;

    let _ = fs::remove_file(&staged);
    Ok(())
}

/// Where the download waits before it is moved into place.
pub fn staging_path(version: &str) -> PathBuf {
    std::env::temp_dir().join(format!("camviewer-{version}-{}", std::process::id()))
}

/// Whether `candidate` is a later version than `current`.
///
/// Compares dot-separated numbers, so `0.10.0` beats `0.9.0`. A build with no
/// version, or a tag that is not a number, never counts as newer.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let parse = |text: &str| -> Option<Vec<u64>> {
        text.split(['.', '-', '+'])
            .take(3)
            .map(|part| part.parse::<u64>().ok())
            .collect::<Option<Vec<u64>>>()
    };

    match (parse(candidate), parse(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{asset_name, is_newer, release_from_json};

    #[test]
    fn a_later_version_is_offered_and_an_equal_or_older_one_is_not() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
    }

    #[test]
    fn a_tag_that_is_not_a_version_is_never_offered() {
        assert!(!is_newer("latest", "0.1.0"));
        assert!(!is_newer("", "0.1.0"));
        assert!(!is_newer("v0.2", "0.1.0"));
    }

    /// A release reply shaped like GitHub's.
    fn payload(tag: &str, assets: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag,
            "body": "### Fixed\n\n- Something.",
            "assets": assets
        })
    }

    fn asset(name: &str) -> serde_json::Value {
        serde_json::json!([{
            "name": name,
            "browser_download_url": "https://example.invalid/build",
            "size": 1234
        }])
    }

    #[test]
    fn a_newer_release_with_a_build_for_this_machine_is_offered() {
        let release = release_from_json(
            &payload("v0.4.0", asset("camviewer-test")),
            "0.3.0",
            "camviewer-test",
        )
        .expect("a usable reply")
        .expect("newer");

        assert_eq!(release.version, "0.4.0");
        assert_eq!(release.url, "https://example.invalid/build");
        assert_eq!(release.size, 1234);
        assert!(release.notes.contains("Something"));
    }

    #[test]
    fn the_same_release_is_not_offered_again() {
        let wanted = asset_name();
        let reply = release_from_json(&payload("v0.3.0", asset(&wanted)), "0.3.0", &wanted)
            .expect("a usable reply");
        assert!(reply.is_none());
    }

    #[test]
    fn a_release_without_a_build_for_this_machine_names_the_one_it_wanted() {
        let error = release_from_json(
            &payload("v0.4.0", asset("camviewer-other")),
            "0.3.0",
            "camviewer-mine",
        )
        .expect_err("no build for this machine");

        assert!(error.contains("camviewer-mine"), "{error}");
        assert!(error.contains("v0.4.0"), "{error}");
    }

    #[test]
    fn a_reply_without_a_tag_or_files_is_an_error_not_a_crash() {
        assert!(release_from_json(&serde_json::json!({}), "0.3.0", "x").is_err());
        assert!(
            release_from_json(&serde_json::json!({"tag_name": "v0.4.0"}), "0.3.0", "x").is_err()
        );
    }

    #[test]
    fn an_asset_without_a_download_address_is_an_error() {
        let assets = serde_json::json!([{ "name": "camviewer-mine", "size": 1 }]);
        let error = release_from_json(&payload("v0.4.0", assets), "0.3.0", "camviewer-mine")
            .expect_err("no address");
        assert!(error.contains("download address"), "{error}");
    }

    #[test]
    fn a_body_that_is_missing_or_not_text_leaves_the_notes_empty() {
        let release = release_from_json(
            &serde_json::json!({
                "tag_name": "v0.4.0",
                "body": serde_json::Value::Null,
                "assets": asset("camviewer-mine")
            }),
            "0.3.0",
            "camviewer-mine",
        )
        .expect("a usable reply")
        .expect("newer");

        assert!(release.notes.is_empty());
    }

    #[test]
    fn the_asset_name_names_this_platform() {
        let name = asset_name();
        assert!(name.starts_with("camviewer-"), "{name}");
        assert!(name.contains(std::env::consts::ARCH), "{name}");
        if cfg!(windows) {
            assert!(name.ends_with(".exe"), "{name}");
        } else {
            assert!(!name.ends_with(".exe"), "{name}");
        }
    }
}
