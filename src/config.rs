//! Camera list persistence: one small JSON file in the user config directory.

use std::fs;
use std::path::PathBuf;

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
    let Ok(text) = fs::read_to_string(config_file()) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// Writes the camera list. Failures are ignored: losing the file must not
/// stop the viewer from running.
pub fn save(cameras: &[CameraConfig]) {
    let path = config_file();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(cameras) {
        let _ = fs::write(path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::CameraConfig;

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
    fn a_file_without_the_low_resolution_field_still_loads() {
        let text = r#"[{"name":"Old","url":"rtsp://cam/stream1"}]"#;
        let cameras: Vec<CameraConfig> = serde_json::from_str(text).expect("decode");

        assert_eq!(cameras.len(), 1);
        assert_eq!(cameras[0].url_sd, None);
    }
}
