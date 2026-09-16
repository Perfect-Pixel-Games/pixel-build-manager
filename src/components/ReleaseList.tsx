import type { Release } from "../api/projects";

type Props = {
  releases: Release[];
  activeReleaseTag: string | null;
  activeAssetName: string | null;
  cachedAssetIds: Set<number>;
  onSync: (release: Release, assetId: number) => void;
  onDelete: (release: Release, assetId: number) => void;
};

export function ReleaseList({
  releases,
  activeReleaseTag,
  activeAssetName,
  cachedAssetIds,
  onSync,
  onDelete,
}: Props) {
  return (
    <ul>
      {releases.map((release) => (
        <li key={release.id}>
          <h3>
            {release.name ?? release.tag_name} {release.prerelease ? "(prerelease)" : ""}
          </h3>
          <ul>
            {release.assets.map((asset) => (
              <li key={asset.id}>
                {asset.name}
                {release.tag_name === activeReleaseTag && asset.name === activeAssetName && (
                  <strong> (Active)</strong>
                )}
                <button aria-label={`Sync ${asset.name}`} onClick={() => onSync(release, asset.id)}>
                  Sync
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
        </li>
      ))}
    </ul>
  );
}
