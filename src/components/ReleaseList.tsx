import { useState } from "react";
import type { Release, ReleaseAsset } from "../api/projects";

type Props = {
  releases: Release[];
  activeReleaseTag: string | null;
  activeAssetName: string | null;
  cachedAssetIds: Set<number>;
  onSync: (release: Release, assetId: number) => void;
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

function optionLabel({ release, asset }: Option): string {
  const releaseLabel = `${release.name ?? release.tag_name}${release.prerelease ? " (prerelease)" : ""}`;
  return `${releaseLabel} — ${displayAssetName(asset.name)}`;
}

export function ReleaseList({
  releases,
  activeReleaseTag,
  activeAssetName,
  cachedAssetIds,
  onSync,
  onDelete,
}: Props) {
  const [open, setOpen] = useState(false);

  const options: Option[] = releases.flatMap((release) =>
    release.assets.map((asset) => ({
      release,
      asset,
      isActive: release.tag_name === activeReleaseTag && asset.name === activeAssetName,
    })),
  );

  const activeOption = options.find((option) => option.isActive);

  return (
    <div>
      <button aria-expanded={open} onClick={() => setOpen((prev) => !prev)}>
        {activeOption ? optionLabel(activeOption) : "No active build"} {open ? "▴" : "▾"}
      </button>
      {open && (
        <ul>
          {options.map(({ release, asset, isActive }) => (
            <li key={`${release.id}-${asset.id}`}>
              <button onClick={() => onSync(release, asset.id)}>
                {optionLabel({ release, asset, isActive })}
                {isActive && <strong> (Active)</strong>}
              </button>
              <button
                aria-label={`Sync ${asset.name}`}
                title={cachedAssetIds.has(asset.id) ? "Already downloaded -- check for a fresh copy" : "Sync"}
                onClick={() => onSync(release, asset.id)}
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
  );
}
