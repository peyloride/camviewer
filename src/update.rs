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

    let tag = release["tag_name"]
        .as_str()
        .ok_or("the newest release has no tag")?;
    let version = tag.trim_start_matches('v').to_string();

    if !is_newer(&version, current) {
        return Ok(None);
    }

    let wanted = asset_name();
    let asset = release["assets"]
        .as_array()
        .ok_or("the newest release lists no files")?
        .iter()
        .find(|asset| asset["name"].as_str() == Some(wanted.as_str()))
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
    use super::{asset_name, is_newer};

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
