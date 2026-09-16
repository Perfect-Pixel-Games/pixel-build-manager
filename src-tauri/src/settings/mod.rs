use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProjectSettings {
    pub favorite: bool,
    pub active_release_tag: Option<String>,
    pub active_asset_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub workspace_root: Option<PathBuf>,
    #[serde(default)]
    pub projects: HashMap<String, ProjectSettings>,
}

impl Settings {
    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self).expect("Settings must serialize");
        std::fs::write(path, contents)
    }

    pub fn set_favorite(&mut self, project_key: &str, favorite: bool) {
        self.projects
            .entry(project_key.to_string())
            .or_default()
            .favorite = favorite;
    }

    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.workspace_root = Some(root);
    }

    pub fn set_active_release(&mut self, project_key: &str, release_tag: &str, asset_name: &str) {
        let project = self.projects.entry(project_key.to_string()).or_default();
        project.active_release_tag = Some(release_tag.to_string());
        project.active_asset_name = Some(asset_name.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_from_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let settings = Settings::load_from(&path);

        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");

        let mut projects = HashMap::new();
        projects.insert(
            "pixel-perfect/last-beacon".to_string(),
            ProjectSettings {
                favorite: true,
                active_release_tag: Some("0.2.14".to_string()),
                active_asset_name: Some("last-beacon-windows-x64-shipping.zip".to_string()),
            },
        );
        let settings = Settings {
            workspace_root: Some(PathBuf::from("D:\\Builds")),
            projects,
        };

        settings.save_to(&path).unwrap();
        let loaded = Settings::load_from(&path);

        assert_eq!(loaded, settings);
    }

    #[test]
    fn set_favorite_creates_entry_if_missing() {
        let mut settings = Settings::default();

        settings.set_favorite("pixel-perfect/last-beacon", true);

        assert!(settings.projects["pixel-perfect/last-beacon"].favorite);

        settings.set_favorite("pixel-perfect/last-beacon", false);

        assert!(!settings.projects["pixel-perfect/last-beacon"].favorite);
    }

    #[test]
    fn set_workspace_root_updates_the_field() {
        let mut settings = Settings::default();

        settings.set_workspace_root(PathBuf::from("D:\\Builds"));

        assert_eq!(settings.workspace_root, Some(PathBuf::from("D:\\Builds")));
    }

    #[test]
    fn set_active_release_records_tag_and_asset() {
        let mut settings = Settings::default();

        settings.set_active_release("org/repo", "0.2.14", "build-shipping.zip");

        let project = &settings.projects["org/repo"];
        assert_eq!(project.active_release_tag, Some("0.2.14".to_string()));
        assert_eq!(
            project.active_asset_name,
            Some("build-shipping.zip".to_string())
        );
    }
}
