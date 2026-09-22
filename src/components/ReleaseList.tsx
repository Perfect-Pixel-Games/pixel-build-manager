import { useEffect, useMemo, useState } from "react";
import type { Release, ReleaseAsset } from "../api/projects";

type Props = {
  releases: Release[];
  activeReleaseTag: string | null;
  activeAssetName: string | null;
  cachedAssetIds: Set<number>;
  /** Picking an option activates it: downloads/verifies it, then extracts it as the active build. */
  onSelect: (release: Release, assetId: number) => void;
  /** Sync/Check only downloads/verifies the cached copy -- it never changes the active build. */
  onCheck: (release: Release, assetId: number) => void;
  onDelete: (release: Release, assetId: number) => void;
  /** True while a download/check is in flight -- every control here is disabled, since none of them are safe to act on mid-operation. */
  disabled: boolean;
};

type Option = {
  release: Release;
  asset: ReleaseAsset;
  isActive: boolean;
};

// Checked longest-first so ".tar.gz" is stripped whole rather than leaving
// a dangling ".tar" behind.
const KNOWN_ARCHIVE_EXTENSIONS = [".tar.gz", ".zip"];

function displayAssetName(assetName: string): string {
  const lower = assetName.toLowerCase();
  const extension = KNOWN_ARCHIVE_EXTENSIONS.find((ext) => lower.endsWith(ext));
  return extension ? assetName.slice(0, -extension.length) : assetName;
}

function releaseLabel(release: Release): string {
  return `${release.name ?? release.tag_name}${release.prerelease ? " (prerelease)" : ""}`;
}

export function ReleaseList({
  releases,
  activeReleaseTag,
  activeAssetName,
  cachedAssetIds,
  onSelect,
  onCheck,
  onDelete,
  disabled,
}: Props) {
  const [open, setOpen] = useState(false);
  const [buildType, setBuildType] = useState<string | null>(null);
  const [showReleases, setShowReleases] = useState(true);
  const [showPrereleases, setShowPrereleases] = useState(true);

  // A project's asset file name is stable across every release of that
  // variant (only the release/tag differs between versions of "the same
  // build"), so the set of distinct asset names a project has ever
  // published *is* its set of build types -- no per-project naming
  // convention to parse, which matters since every project's naming
  // differs (test/shipping, platform, arch, etc.).
  const buildTypes = useMemo(() => {
    const names = new Set<string>();
    for (const release of releases) {
      for (const asset of release.assets) {
        names.add(asset.name);
      }
    }
    return Array.from(names).sort((a, b) => a.localeCompare(b));
  }, [releases]);

  // Default to the active build's type when there is one; otherwise, only
  // auto-pick when there's no ambiguity (a single build type). Never
  // overrides a choice the user already made.
  useEffect(() => {
    if (buildType !== null) {
      return;
    }
    if (activeAssetName && buildTypes.includes(activeAssetName)) {
      setBuildType(activeAssetName);
    } else if (buildTypes.length === 1) {
      setBuildType(buildTypes[0]);
    }
  }, [buildTypes, activeAssetName, buildType]);

  // Every option for the selected build type, across all releases --
  // independent of the release/prerelease display filter below, since that
  // filter must never affect what counts as "active".
  const optionsForBuildType: Option[] = releases.flatMap((release) =>
    release.assets
      .filter((asset) => asset.name === buildType)
      .map((asset) => ({
        release,
        asset,
        isActive: release.tag_name === activeReleaseTag && asset.name === activeAssetName,
      })),
  );

  const activeOption = optionsForBuildType.find((option) => option.isActive);

  // Display-only: narrows which of the above options are rendered in the
  // list, but never influences activation.
  const visibleOptions = optionsForBuildType.filter(({ release }) =>
    release.prerelease ? showPrereleases : showReleases,
  );

  // Switching build type re-activates the currently active release under
  // the new type (same CL, different variant) when that variant exists --
  // since selecting any build type already downloads every type for that
  // release, this is just an extract-and-switch, not a fresh download.
  const handleBuildTypeChange = (value: string) => {
    const newBuildType = value || null;
    setBuildType(newBuildType);

    if (!newBuildType || !activeReleaseTag || newBuildType === activeAssetName) {
      return;
    }
    const activeRelease = releases.find((release) => release.tag_name === activeReleaseTag);
    const matchingAsset = activeRelease?.assets.find((asset) => asset.name === newBuildType);
    if (activeRelease && matchingAsset) {
      onSelect(activeRelease, matchingAsset.id);
    }
  };

  return (
    <div>
      <select
        aria-label="Build type"
        value={buildType ?? ""}
        disabled={disabled}
        onChange={(event) => handleBuildTypeChange(event.target.value)}
      >
        <option value="" disabled>
          Select a build type
        </option>
        {buildTypes.map((name) => (
          <option key={name} value={name}>
            {displayAssetName(name)}
          </option>
        ))}
      </select>
      {buildType === null ? (
        buildTypes.length > 0 && <p>Select a build type to see available versions.</p>
      ) : (
        <div>
          <fieldset>
            <legend>Show</legend>
            <label>
              <input
                type="checkbox"
                checked={showReleases}
                disabled={disabled}
                onChange={(event) => setShowReleases(event.target.checked)}
              />
              Releases
            </label>
            <label>
              <input
                type="checkbox"
                checked={showPrereleases}
                disabled={disabled}
                onChange={(event) => setShowPrereleases(event.target.checked)}
              />
              Prereleases
            </label>
          </fieldset>
          <button aria-expanded={open} disabled={disabled} onClick={() => setOpen((prev) => !prev)}>
            {activeOption ? releaseLabel(activeOption.release) : "No active build"} {open ? "▴" : "▾"}
          </button>
          {open && (
            <ul>
              {visibleOptions.map(({ release, asset, isActive }) => (
                <li key={`${release.id}-${asset.id}`}>
                  <button
                    aria-label={releaseLabel(release)}
                    disabled={disabled}
                    onClick={() => onSelect(release, asset.id)}
                  >
                    {releaseLabel(release)}
                    {isActive && <strong> (Active)</strong>}
                  </button>
                  <button
                    aria-label={`Sync ${asset.name}`}
                    title={cachedAssetIds.has(asset.id) ? "Already downloaded -- check for a fresh copy" : "Sync"}
                    disabled={disabled}
                    onClick={() => onCheck(release, asset.id)}
                  >
                    {cachedAssetIds.has(asset.id) ? "Check" : "Sync"}
                  </button>
                  {cachedAssetIds.has(asset.id) && (
                    <button
                      aria-label={`Delete downloaded ${asset.name}`}
                      disabled={disabled}
                      onClick={() => onDelete(release, asset.id)}
                    >
                      Delete
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}
