use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProjectSettings {
    pub favorite: bool,
    pub selected_release_tag: Option<String>,
    #[serde(default)]
    pub ticked_configs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub workspace_root: Option<PathBuf>,
    #[serde(default)]
    pub projects: HashMap<String, ProjectSettings>,
    #[serde(default)]
    pub bound_projects: Vec<String>,
    #[serde(default)]
    pub theme: Theme,
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

    /// Appends `project_key` to the tab order if it isn't already bound.
    /// Binding an already-bound project is a no-op rather than moving it to
    /// the end -- rebinding must never reorder existing tabs.
    pub fn bind_project(&mut self, project_key: &str) {
        if !self.bound_projects.iter().any(|p| p == project_key) {
            self.bound_projects.push(project_key.to_string());
        }
    }

    /// Removes `project_key` from the tab order. Leaves its `ProjectSettings`
    /// entry (selected release, ticked configs, favorite) untouched, so
    /// rebinding later restores where the user left off.
    pub fn unbind_project(&mut self, project_key: &str) {
        self.bound_projects.retain(|p| p != project_key);
    }

    pub fn set_selected_release(&mut self, project_key: &str, release_tag: &str) {
        self.projects
            .entry(project_key.to_string())
            .or_default()
            .selected_release_tag = Some(release_tag.to_string());
    }

    pub fn set_ticked_configs(&mut self, project_key: &str, configs: Vec<String>) {
        self.projects
            .entry(project_key.to_string())
            .or_default()
            .ticked_configs = configs;
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
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
    fn default_theme_is_system() {
        assert_eq!(Settings::default().theme, Theme::System);
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
                selected_release_tag: Some("0.2.14".to_string()),
                ticked_configs: vec!["shipping.zip".to_string()],
            },
        );
        let settings = Settings {
            workspace_root: Some(PathBuf::from("D:\\Builds")),
            projects,
            bound_projects: vec!["pixel-perfect/last-beacon".to_string()],
            theme: Theme::Dark,
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
    fn bind_project_appends_in_order_and_is_idempotent() {
        let mut settings = Settings::default();

        settings.bind_project("org/repo-a");
        settings.bind_project("org/repo-b");
        settings.bind_project("org/repo-a");

        assert_eq!(settings.bound_projects, vec!["org/repo-a", "org/repo-b"]);
    }

    #[test]
    fn unbind_project_removes_it_and_is_a_no_op_when_absent() {
        let mut settings = Settings::default();
        settings.bind_project("org/repo-a");
        settings.bind_project("org/repo-b");

        settings.unbind_project("org/repo-a");
        settings.unbind_project("org/does-not-exist");

        assert_eq!(settings.bound_projects, vec!["org/repo-b"]);
    }

    #[test]
    fn set_selected_release_records_the_tag() {
        let mut settings = Settings::default();

        settings.set_selected_release("org/repo", "0.2.14");

        assert_eq!(
            settings.projects["org/repo"].selected_release_tag,
            Some("0.2.14".to_string())
        );
    }

    #[test]
    fn set_ticked_configs_replaces_the_list() {
        let mut settings = Settings::default();
        settings.set_ticked_configs("org/repo", vec!["a.zip".to_string()]);

        settings.set_ticked_configs("org/repo", vec!["b.zip".to_string(), "c.zip".to_string()]);

        assert_eq!(
            settings.projects["org/repo"].ticked_configs,
            vec!["b.zip".to_string(), "c.zip".to_string()]
        );
    }

    #[test]
    fn set_theme_updates_the_field() {
        let mut settings = Settings::default();

        settings.set_theme(Theme::Dark);

        assert_eq!(settings.theme, Theme::Dark);
    }

    #[test]
    fn theme_serializes_as_lowercase_strings() {
        assert_eq!(serde_json::to_string(&Theme::Light).unwrap(), "\"light\"");
        assert_eq!(serde_json::to_string(&Theme::Dark).unwrap(), "\"dark\"");
        assert_eq!(serde_json::to_string(&Theme::System).unwrap(), "\"system\"");
    }
}
