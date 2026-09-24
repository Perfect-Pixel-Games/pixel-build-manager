import { useMemo, useState } from "react";
import type { Release, ReleaseAsset } from "../api/projects";

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
        names.add(asset.name);
      }
    }
    return Array.from(names).sort((a, b) => a.localeCompare(b));
  }, [releases]);

  const visibleReleases = useMemo(() => {
    const term = search.toLowerCase();
    return releases
      .filter((release) => !release.prerelease)
      .filter((release) => (release.name ?? release.tag_name).toLowerCase().includes(term));
  }, [releases, search]);

  const selectedRelease = releases.find((release) => release.tag_name === selectedReleaseTag) ?? null;

  const availableTickedAssets: ReleaseAsset[] = selectedRelease
    ? selectedRelease.assets.filter((asset) => tickedConfigs.includes(asset.name))
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
    <div>
      <input
        aria-label="Search releases"
        value={search}
        disabled={disabled}
        onChange={(event) => setSearch(event.target.value)}
      />
      <ul>
        {visibleReleases.map((release) => (
          <li key={release.id}>
            <button
              aria-pressed={release.tag_name === selectedReleaseTag}
              disabled={disabled}
              onClick={() => onSelectRelease(release.tag_name)}
            >
              {release.name ?? release.tag_name}
            </button>
          </li>
        ))}
      </ul>
      <fieldset>
        <legend>Build configs</legend>
        {buildConfigs.map((config) => {
          const availableForSelected =
            !selectedRelease || selectedRelease.assets.some((asset) => asset.name === config);
          return (
            <label key={config}>
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
      </fieldset>
      <button disabled={disabled || availableTickedAssets.length === 0} onClick={handleSyncClick}>
        {fullySynced ? "✓ Synced" : "Sync"}
      </button>
    </div>
  );
}
