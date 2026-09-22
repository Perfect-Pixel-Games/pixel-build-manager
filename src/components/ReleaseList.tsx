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
}: Props) {
  const [open, setOpen] = useState(false);
  const [buildType, setBuildType] = useState<string | null>(null);

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

  const options: Option[] = releases.flatMap((release) =>
    release.assets
      .filter((asset) => asset.name === buildType)
      .map((asset) => ({
        release,
        asset,
        isActive: release.tag_name === activeReleaseTag && asset.name === activeAssetName,
      })),
  );

  const activeOption = options.find((option) => option.isActive);

  return (
    <div>
      <select
        aria-label="Build type"
        value={buildType ?? ""}
        onChange={(event) => setBuildType(event.target.value || null)}
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
          <button aria-expanded={open} onClick={() => setOpen((prev) => !prev)}>
            {activeOption ? releaseLabel(activeOption.release) : "No active build"} {open ? "▴" : "▾"}
          </button>
          {open && (
            <ul>
              {options.map(({ release, asset, isActive }) => (
                <li key={`${release.id}-${asset.id}`}>
                  <button aria-label={releaseLabel(release)} onClick={() => onSelect(release, asset.id)}>
                    {releaseLabel(release)}
                    {isActive && <strong> (Active)</strong>}
                  </button>
                  <button
                    aria-label={`Sync ${asset.name}`}
                    title={cachedAssetIds.has(asset.id) ? "Already downloaded -- check for a fresh copy" : "Sync"}
                    onClick={() => onCheck(release, asset.id)}
                  >
                    {cachedAssetIds.has(asset.id) ? "Check" : "Sync"}
                  </button>
                  {cachedAssetIds.has(asset.id) && (
                    <button
                      aria-label={`Delete downloaded ${asset.name}`}
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
