use super::cache::LatestChannel;
use crate::github::client::ReleaseSummary;

const SEPARATORS: [char; 3] = ['.', '_', '-'];

/// Mirrors the frontend's `configTemplate()` in `BuildBrowser.tsx`: strips
/// the release's own tag out of an asset name, so a project whose CI embeds
/// the version in the artifact filename (e.g. Tauri/Electron installers)
/// still collapses to the one stable config identity `ticked_configs` is
/// keyed by, rather than registering a new "config" every release. This
/// port exists because a latest-mode sync must resolve that same identity
/// with no frontend involved (it can run entirely from a background poll).
fn config_template(release_tag: &str, asset_name: &str) -> String {
    if release_tag.is_empty() {
        return asset_name.to_string();
    }
    let stripped = asset_name.replace(release_tag, "");

    let mut collapsed = String::with_capacity(stripped.len());
    let mut previous: Option<char> = None;
    for c in stripped.chars() {
        if SEPARATORS.contains(&c) && previous == Some(c) {
            continue;
        }
        collapsed.push(c);
        previous = Some(c);
    }

    collapsed
        .trim_matches(|c| SEPARATORS.contains(&c))
        .to_string()
}

/// Picks the most recently published release matching `channel` (release ==
/// non-prerelease, prerelease == prerelease) out of `releases`. Sorts by
/// `published_at` explicitly rather than trusting the GitHub API's response
/// ordering. Returns `None` if no release in that channel exists yet (e.g.
/// a project with no prerelease published so far).
fn resolve_latest_release(
    releases: &[ReleaseSummary],
    channel: LatestChannel,
) -> Option<&ReleaseSummary> {
    let want_prerelease = matches!(channel, LatestChannel::Prerelease);
    let mut matching: Vec<&ReleaseSummary> = releases
        .iter()
        .filter(|release| release.prerelease == want_prerelease)
        .collect();
    matching.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    matching.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(id: u64, tag: &str, prerelease: bool, published_at: Option<&str>) -> ReleaseSummary {
        ReleaseSummary {
            id,
            tag_name: tag.to_string(),
            name: None,
            prerelease,
            published_at: published_at.map(|s| s.to_string()),
            assets: Vec::new(),
        }
    }

    #[test]
    fn config_template_strips_the_release_tag_and_collapses_separators() {
        assert_eq!(
            config_template("0.0.3", "pixel-build-manager_0.0.3_x64-setup.exe"),
            "pixel-build-manager_x64-setup.exe"
        );
    }

    #[test]
    fn config_template_is_the_identity_when_the_tag_is_not_in_the_name() {
        assert_eq!(
            config_template("0.2.14", "last-beacon-windows-x64-shipping.tar.gz"),
            "last-beacon-windows-x64-shipping.tar.gz"
        );
    }

    #[test]
    fn resolve_latest_release_picks_the_most_recently_published_match_for_the_channel() {
        let releases = vec![
            release(1, "0.2.13", false, Some("2026-08-01T00:00:00Z")),
            release(2, "0.2.14", false, Some("2026-09-01T00:00:00Z")),
            release(3, "0.3.0-rc1", true, Some("2026-09-10T00:00:00Z")),
        ];

        let latest_release = resolve_latest_release(&releases, LatestChannel::Release).unwrap();
        let latest_prerelease =
            resolve_latest_release(&releases, LatestChannel::Prerelease).unwrap();

        assert_eq!(latest_release.tag_name, "0.2.14");
        assert_eq!(latest_prerelease.tag_name, "0.3.0-rc1");
    }

    #[test]
    fn resolve_latest_release_returns_none_when_no_release_matches_the_channel() {
        let releases = vec![release(1, "0.2.14", false, Some("2026-09-01T00:00:00Z"))];

        assert!(resolve_latest_release(&releases, LatestChannel::Prerelease).is_none());
    }
}
