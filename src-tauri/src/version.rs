use crate::updater::channel::Channel;

/// Formats the build-identity text shown in the app footer. `version` is
/// the app's compiled-in version (see `app.package_info().version`), which
/// CI overwrites with the release tag before building -- see the "Write
/// version into tauri.conf.json" step in
/// `.github/workflows/pixel-build-manager-build.yml`.
pub fn format_version_label(channel: Option<Channel>, version: &str) -> String {
    match channel {
        Some(Channel::Release) => format!("Release {version}"),
        Some(Channel::Prerelease) => format!("Pre-release {version}"),
        None => "Development".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::channel::Channel;

    #[test]
    fn formats_release_label() {
        assert_eq!(
            format_version_label(Some(Channel::Release), "0.4.0"),
            "Release 0.4.0"
        );
    }

    #[test]
    fn formats_prerelease_label() {
        assert_eq!(
            format_version_label(Some(Channel::Prerelease), "0.3.2"),
            "Pre-release 0.3.2"
        );
    }

    #[test]
    fn formats_development_label_when_channel_unknown() {
        assert_eq!(format_version_label(None, "0.1.0"), "Development");
    }
}
