import { useMemo, useState } from "react";
import type { Release, ReleaseAsset } from "../api/projects";
import type { SyncMode } from "../api/sync";

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
//
// Mirrored in Rust as `config_template()` in `sync/latest.rs`, since a
// latest-mode sync must resolve the same identity without any frontend
// involved.
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

// Picks the most recently published release matching `prerelease`, mirroring
// the backend's `resolve_latest_release()` so the UI's "available configs"
// checkboxes agree with whatever the backend will actually sync.
function resolveLatest(releases: Release[], prerelease: boolean): Release | null {
  const matching = releases.filter((release) => release.prerelease === prerelease);
  const sorted = [...matching].sort((a, b) => {
    if (a.published_at === b.published_at) return 0;
    if (a.published_at === null) return 1;
    if (b.published_at === null) return -1;
    return b.published_at.localeCompare(a.published_at);
  });
  return sorted[0] ?? null;
}

type Props = {
  releases: Release[];
  syncMode: SyncMode;
  selectedReleaseTag: string | null;
  onSelectRelease: (tag: string) => void;
  onSelectLatest: (mode: "latest_release" | "latest_prerelease") => void;
  tickedConfigs: string[];
  onToggleConfig: (configName: string, ticked: boolean) => void;
  /** Configs already extracted for the *effective* release: the selected
   * release in manual mode, or whatever the active latest mode currently
   * resolves to. */
  syncedConfigs: string[];
  onSync: (release: Release, assets: ReleaseAsset[]) => void;
  onSyncLatest: () => void;
  disabled: boolean;
};

export function BuildBrowser({
  releases,
  syncMode,
  selectedReleaseTag,
  onSelectRelease,
  onSelectLatest,
  tickedConfigs,
  onToggleConfig,
  syncedConfigs,
  onSync,
  onSyncLatest,
  disabled,
}: Props) {
  const [search, setSearch] = useState("");
  const isLatestMode = syncMode !== "manual";

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

  const effectiveRelease: Release | null = isLatestMode
    ? resolveLatest(releases, syncMode === "latest_prerelease")
    : releases.find((release) => release.tag_name === selectedReleaseTag) ?? null;

  const availableTickedAssets: ReleaseAsset[] = effectiveRelease
    ? effectiveRelease.assets.filter((asset) =>
        tickedConfigs.includes(configTemplate(effectiveRelease, asset.name)),
      )
    : [];
  const missingAssets = availableTickedAssets.filter((asset) => !syncedConfigs.includes(asset.name));

  const fullySynced = isLatestMode
    ? tickedConfigs.length > 0 && tickedConfigs.every((config) => syncedConfigs.includes(config))
    : availableTickedAssets.length > 0 && missingAssets.length === 0;

  const syncDisabled = disabled || (isLatestMode ? tickedConfigs.length === 0 : availableTickedAssets.length === 0);

  const handleSyncClick = () => {
    if (isLatestMode) {
      if (tickedConfigs.length === 0) {
        return;
      }
      onSyncLatest();
      return;
    }
    if (!effectiveRelease || availableTickedAssets.length === 0) {
      return;
    }
    onSync(effectiveRelease, fullySynced ? availableTickedAssets : missingAssets);
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
        <button
          className="release-row release-row--pinned"
          aria-pressed={syncMode === "latest_release"}
          disabled={disabled}
          onClick={() => onSelectLatest("latest_release")}
        >
          Latest release
        </button>
        <button
          className="release-row release-row--pinned"
          aria-pressed={syncMode === "latest_prerelease"}
          disabled={disabled}
          onClick={() => onSelectLatest("latest_prerelease")}
        >
          Latest prerelease
        </button>
        {visibleReleases.length === 0 ? (
          <p className="release-list__empty">No releases found.</p>
        ) : (
          visibleReleases.map((release) => (
            <button
              key={release.id}
              className="release-row"
              aria-label={releaseLabel(release)}
              aria-pressed={!isLatestMode && release.tag_name === selectedReleaseTag}
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
              !effectiveRelease ||
              effectiveRelease.assets.some((asset) => configTemplate(effectiveRelease, asset.name) === config);
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
        disabled={syncDisabled}
        onClick={handleSyncClick}
      >
        {fullySynced ? "✓ Synced" : "Sync"}
      </button>
    </div>
  );
}
