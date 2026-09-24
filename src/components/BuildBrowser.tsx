import { useMemo, useState } from "react";
import type { Release, ReleaseAsset } from "../api/projects";

function releaseLabel(release: Release): string {
  const base = release.name ?? release.tag_name;
  return release.prerelease ? `${base} (prerelease)` : base;
}

// A build config's *identity* shouldn't include the release's own version --
// otherwise a project whose CI embeds the version in the artifact filename
// (e.g. Tauri/Electron installers: "app_0.0.3_x64-setup.exe") gets a brand
// new "config" every single release, instead of the one stable config it
// actually is. Stripping the release's own tag out of the asset name before
// treating it as a config identity collapses those back into one, the same
// way a project like last-beacon (whose asset names never vary release to
// release, e.g. "last-beacon-windows-x64-shipping.tar.gz") already worked.
function configTemplate(release: Release, assetName: string): string {
  if (!release.tag_name) {
    return assetName;
  }
  return assetName
    .split(release.tag_name)
    .join("")
    .replace(/([._-])\1+/g, "$1")
    .replace(/^[._-]+|[._-]+$/g, "");
}

type Props = {
  releases: Release[];
  selectedReleaseTag: string | null;
  onSelectRelease: (tag: string) => void;
  tickedConfigs: string[];
  onToggleConfig: (configName: string, ticked: boolean) => void;
  /** Configs already extracted for the *selected* release. */
  syncedConfigs: string[];
  onSync: (release: Release, assets: ReleaseAsset[]) => void;
  disabled: boolean;
};

export function BuildBrowser({
  releases,
  selectedReleaseTag,
  onSelectRelease,
  tickedConfigs,
  onToggleConfig,
  syncedConfigs,
  onSync,
  disabled,
}: Props) {
  const [search, setSearch] = useState("");

  // Every build config the project has ever produced, across every release
  // -- including prereleases -- deliberately not scoped to the releases
  // visible below, so a config that's so far only shipped in a prerelease
  // still shows up as a future sync target once a real release adds it.
  const buildConfigs = useMemo(() => {
    const names = new Set<string>();
    for (const release of releases) {
      for (const asset of release.assets) {
        names.add(configTemplate(release, asset.name));
      }
    }
    return Array.from(names).sort((a, b) => a.localeCompare(b));
  }, [releases]);

  const visibleReleases = useMemo(() => {
    const term = search.toLowerCase();
    return releases.filter((release) => (release.name ?? release.tag_name).toLowerCase().includes(term));
  }, [releases, search]);

  const selectedRelease = releases.find((release) => release.tag_name === selectedReleaseTag) ?? null;

  const availableTickedAssets: ReleaseAsset[] = selectedRelease
    ? selectedRelease.assets.filter((asset) =>
        tickedConfigs.includes(configTemplate(selectedRelease, asset.name)),
      )
    : [];
  const missingAssets = availableTickedAssets.filter((asset) => !syncedConfigs.includes(asset.name));
  const fullySynced = availableTickedAssets.length > 0 && missingAssets.length === 0;

  const handleSyncClick = () => {
    if (!selectedRelease || availableTickedAssets.length === 0) {
      return;
    }
    onSync(selectedRelease, fullySynced ? availableTickedAssets : missingAssets);
  };

  return (
    <div className="build-browser">
      <input
        className="build-browser__search"
        aria-label="Search releases"
        placeholder="Search releases..."
        value={search}
        disabled={disabled}
        onChange={(event) => setSearch(event.target.value)}
      />
      <div className="release-list">
        {visibleReleases.length === 0 ? (
          <p className="release-list__empty">No releases found.</p>
        ) : (
          visibleReleases.map((release) => (
            <button
              key={release.id}
              className="release-row"
              aria-label={releaseLabel(release)}
              aria-pressed={release.tag_name === selectedReleaseTag}
              disabled={disabled}
              onClick={() => onSelectRelease(release.tag_name)}
            >
              <span className="release-row__name">{releaseLabel(release)}</span>
              {release.published_at && (
                <span className="release-row__date">
                  {new Date(release.published_at).toLocaleDateString()}
                </span>
              )}
            </button>
          ))
        )}
      </div>
      <fieldset className="build-configs">
        <legend>Build configs</legend>
        <div className="build-configs__list">
          {buildConfigs.map((config) => {
            const availableForSelected =
              !selectedRelease ||
              selectedRelease.assets.some((asset) => configTemplate(selectedRelease, asset.name) === config);
            return (
              <label key={config} className="config-chip">
                <input
                  type="checkbox"
                  aria-label={config}
                  checked={tickedConfigs.includes(config)}
                  disabled={disabled || !availableForSelected}
                  onChange={(event) => onToggleConfig(config, event.target.checked)}
                />
                {config}
              </label>
            );
          })}
        </div>
      </fieldset>
      <button
        className={`sync-button${fullySynced ? " sync-button--synced" : ""}`}
        disabled={disabled || availableTickedAssets.length === 0}
        onClick={handleSyncClick}
      >
        {fullySynced ? "✓ Synced" : "Sync"}
      </button>
    </div>
  );
}
