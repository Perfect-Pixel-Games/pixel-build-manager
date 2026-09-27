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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    #[default]
    Manual,
    LatestRelease,
    LatestPrerelease,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProjectSettings {
    pub favorite: bool,
    pub selected_release_tag: Option<String>,
    #[serde(default)]
    pub ticked_configs: Vec<String>,
    #[serde(default)]
    pub sync_mode: SyncMode,
    /// True when the project was bound by pasting its URL, rather than
    /// picked from the logged-in account's project list. URL-bound
    /// projects survive logging out; account-bound ones don't.
    #[serde(default)]
    pub bound_by_url: bool,
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

    /// Like `bind_project`, but also records how it was bound -- which
    /// decides whether it survives logging out (see `bound_by_url`).
    pub fn bind_project_via(&mut self, project_key: &str, by_url: bool) {
        self.bind_project(project_key);
        self.projects
            .entry(project_key.to_string())
            .or_default()
            .bound_by_url = by_url;
    }

    /// Bound projects that came from the logged-in account's project list,
    /// i.e. every bound project not bound by URL. Projects bound before
    /// binding sources were recorded count as account-bound.
    pub fn account_bound_projects(&self) -> Vec<String> {
        self.bound_projects
            .iter()
            .filter(|key| !self.projects.get(*key).is_some_and(|p| p.bound_by_url))
            .cloned()
            .collect()
    }

    /// Unbinds `project_key` and forgets all its per-project settings
    /// (selected release, ticked configs, favorite, sync mode).
    pub fn remove_project(&mut self, project_key: &str) {
        self.unbind_project(project_key);
        self.projects.remove(project_key);
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

    pub fn set_sync_mode(&mut self, project_key: &str, mode: SyncMode) {
        self.projects
            .entry(project_key.to_string())
            .or_default()
            .sync_mode = mode;
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
                sync_mode: SyncMode::LatestRelease,
                bound_by_url: true,
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

    #[test]
    fn default_sync_mode_is_manual() {
        assert_eq!(ProjectSettings::default().sync_mode, SyncMode::Manual);
    }

    #[test]
    fn sync_mode_serializes_as_snake_case_strings() {
        assert_eq!(
            serde_json::to_string(&SyncMode::Manual).unwrap(),
            "\"manual\""
        );
        assert_eq!(
            serde_json::to_string(&SyncMode::LatestRelease).unwrap(),
            "\"latest_release\""
        );
        assert_eq!(
            serde_json::to_string(&SyncMode::LatestPrerelease).unwrap(),
            "\"latest_prerelease\""
        );
    }

    #[test]
    fn set_sync_mode_creates_entry_if_missing() {
        let mut settings = Settings::default();

        settings.set_sync_mode("org/repo", SyncMode::LatestRelease);

        assert_eq!(
            settings.projects["org/repo"].sync_mode,
            SyncMode::LatestRelease
        );
    }

    #[test]
    fn deserializing_settings_without_a_sync_mode_field_defaults_to_manual() {
        let json = r#"{"projects":{"org/repo":{"favorite":false,"selected_release_tag":null,"ticked_configs":[]}}}"#;

        let settings: Settings = serde_json::from_str(json).unwrap();

        assert_eq!(settings.projects["org/repo"].sync_mode, SyncMode::Manual);
    }

    #[test]
    fn account_bound_projects_excludes_url_bound_ones() {
        let mut settings = Settings::default();
        settings.bind_project_via("org/from-account", false);
        settings.bind_project_via("someone/from-url", true);
        // Bound before sources were recorded: no ProjectSettings entry.
        settings.bind_project("org/legacy");

        assert_eq!(
            settings.account_bound_projects(),
            vec!["org/from-account", "org/legacy"]
        );
    }

    #[test]
    fn rebinding_from_the_account_list_clears_a_stale_url_flag() {
        let mut settings = Settings::default();
        settings.bind_project_via("org/repo", true);
        settings.unbind_project("org/repo");

        settings.bind_project_via("org/repo", false);

        assert_eq!(settings.account_bound_projects(), vec!["org/repo"]);
    }

    #[test]
    fn remove_project_unbinds_and_forgets_its_settings() {
        let mut settings = Settings::default();
        settings.bind_project_via("org/repo", false);
        settings.set_selected_release("org/repo", "1.0");

        settings.remove_project("org/repo");

        assert!(settings.bound_projects.is_empty());
        assert!(!settings.projects.contains_key("org/repo"));
    }

    #[test]
    fn deserializing_settings_without_a_bound_by_url_field_defaults_to_false() {
        let json = r#"{"projects":{"org/repo":{"favorite":false,"selected_release_tag":null}}}"#;

        let settings: Settings = serde_json::from_str(json).unwrap();

        assert!(!settings.projects["org/repo"].bound_by_url);
    }
}
