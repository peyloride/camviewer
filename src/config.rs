//! Camera list persistence: one small JSON file in the user config directory.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Release notes waiting to be shown, written by the updater before it
/// restarts into the new build.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WhatsNew {
    pub version: String,
    pub notes: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraConfig {
    pub name: String,
    /// The full-resolution stream.
    pub url: String,
    /// An optional low-resolution stream for the grid view. Enlarging a camera
    /// switches it back to [`Self::url`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_sd: Option<String>,
}

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("camviewer")
}

/// Where the camera list lives.
pub fn path() -> PathBuf {
    config_dir().join("cameras.json")
}

fn config_file() -> PathBuf {
    path()
}

fn whats_new_file() -> PathBuf {
    config_dir().join("whats-new.json")
}

/// Records the notes for a build that has just been put in place. The next
/// start shows them, once.
pub fn store_whats_new(version: &str, notes: &str) {
    store_whats_new_at(&whats_new_file(), version, notes);
}

/// Records the notes in one file. A release with no notes leaves nothing
/// behind, so no empty popup appears.
pub fn store_whats_new_at(path: &Path, version: &str, notes: &str) {
    if notes.trim().is_empty() {
        return;
    }
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let payload = WhatsNew {
        version: version.to_string(),
        notes: notes.to_string(),
    };
    if let Ok(text) = serde_json::to_string(&payload) {
        let _ = fs::write(path, text);
    }
}

/// Notes left by an update, if any.
pub fn load_whats_new() -> Option<WhatsNew> {
    load_whats_new_from(&whats_new_file())
}

/// Reads the notes from one file.
pub fn load_whats_new_from(path: &Path) -> Option<WhatsNew> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Forgets the notes, so they are shown only once.
pub fn clear_whats_new() {
    clear_whats_new_at(&whats_new_file());
}

/// Removes the notes file.
pub fn clear_whats_new_at(path: &Path) {
    let _ = fs::remove_file(path);
}

/// Reads the saved cameras. A missing or broken file yields an empty list.
pub fn load() -> Vec<CameraConfig> {
    load_from(&config_file())
}

/// Reads a camera list from one file.
pub fn load_from(path: &Path) -> Vec<CameraConfig> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// Writes the camera list. Failures are ignored: losing the file must not
/// stop the viewer from running.
pub fn save(cameras: &[CameraConfig]) {
    save_to(&config_file(), cameras);
}

/// Writes a camera list to one file, creating its directory if needed.
pub fn save_to(path: &Path, cameras: &[CameraConfig]) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(cameras) {
        let _ = fs::write(path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CameraConfig, clear_whats_new_at, load_from, load_whats_new_from, save_to,
        store_whats_new_at,
    };

    #[test]
    fn a_camera_list_survives_a_round_trip() {
        let cameras = vec![
            CameraConfig {
                name: "Salon".into(),
                url: "rtsp://cam/stream1".into(),
                url_sd: Some("rtsp://cam/stream2".into()),
            },
            CameraConfig {
                name: "Rüzgar".into(),
                url: "rtsp://other/stream1".into(),
                url_sd: None,
            },
        ];

        let text = serde_json::to_string_pretty(&cameras).expect("encode");
        let restored: Vec<CameraConfig> = serde_json::from_str(&text).expect("decode");

        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0].name, "Salon");
        assert_eq!(restored[0].url_sd.as_deref(), Some("rtsp://cam/stream2"));
        assert_eq!(restored[1].url_sd, None);
    }

    #[test]
    fn a_saved_list_comes_back_from_disk() {
        let dir = std::env::temp_dir().join(format!("camviewer-test-{}", std::process::id()));
        let path = dir.join("cameras.json");

        let cameras = vec![CameraConfig {
            name: "Salon".into(),
            url: "rtsp://cam/stream1".into(),
            url_sd: Some("rtsp://cam/stream2".into()),
        }];

        save_to(&path, &cameras);
        let restored = load_from(&path);

        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].name, "Salon");
        assert_eq!(restored[0].url_sd.as_deref(), Some("rtsp://cam/stream2"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_file_reads_as_an_empty_list() {
        let path = std::env::temp_dir().join("camviewer-does-not-exist/cameras.json");
        assert!(load_from(&path).is_empty());
    }

    #[test]
    fn release_notes_are_shown_once() {
        let dir = std::env::temp_dir().join(format!("camviewer-notes-{}", std::process::id()));
        let path = dir.join("whats-new.json");

        store_whats_new_at(&path, "0.2.0", "### Added\n\n- Something new.");
        let notes = load_whats_new_from(&path).expect("notes were stored");
        assert_eq!(notes.version, "0.2.0");
        assert!(notes.notes.contains("Something new"));

        // Dismissing removes the file, so the popup does not come back.
        clear_whats_new_at(&path);
        assert!(load_whats_new_from(&path).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_release_without_notes_leaves_no_popup_behind() {
        let dir = std::env::temp_dir().join(format!("camviewer-empty-{}", std::process::id()));
        let path = dir.join("whats-new.json");

        store_whats_new_at(&path, "0.2.1", "   \n  ");
        assert!(load_whats_new_from(&path).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_without_the_low_resolution_field_still_loads() {
        let text = r#"[{"name":"Old","url":"rtsp://cam/stream1"}]"#;
        let cameras: Vec<CameraConfig> = serde_json::from_str(text).expect("decode");

        assert_eq!(cameras.len(), 1);
        assert_eq!(cameras[0].url_sd, None);
    }
}
