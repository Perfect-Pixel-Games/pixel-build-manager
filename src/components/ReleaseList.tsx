import type { Release } from "../api/projects";

type Props = {
  releases: Release[];
  activeAssetName: string | null;
  onSync: (release: Release, assetId: number) => void;
};

export function ReleaseList({ releases, activeAssetName, onSync }: Props) {
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
                {asset.name === activeAssetName && <strong> (Active)</strong>}
                <button aria-label={`Sync ${asset.name}`} onClick={() => onSync(release, asset.id)}>
                  Sync
                </button>
              </li>
            ))}
          </ul>
        </li>
      ))}
    </ul>
  );
}
