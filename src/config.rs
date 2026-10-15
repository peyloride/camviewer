//! Camera list persistence: one small JSON file in the user config directory.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

fn config_file() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("camviewer")
        .join("cameras.json")
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
    use super::{CameraConfig, load_from, save_to};

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
    fn a_file_without_the_low_resolution_field_still_loads() {
        let text = r#"[{"name":"Old","url":"rtsp://cam/stream1"}]"#;
        let cameras: Vec<CameraConfig> = serde_json::from_str(text).expect("decode");

        assert_eq!(cameras.len(), 1);
        assert_eq!(cameras[0].url_sd, None);
    }
}
