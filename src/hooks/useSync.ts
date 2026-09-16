import { useCallback, useEffect, useState } from "react";
import { onSyncProgress, syncReleaseAsset } from "../api/sync";
import type { Release, ReleaseAsset } from "../api/projects";

export type SyncState =
  | { phase: "idle" }
  | { phase: "syncing"; downloaded: number; total: number }
  | { phase: "done" }
  | { phase: "error"; message: string };

export function useSync(projectKey: string) {
  const [state, setState] = useState<SyncState>({ phase: "idle" });

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    onSyncProgress((progress) => {
      if (progress.project_key !== projectKey) {
        return;
      }
      setState({ phase: "syncing", downloaded: progress.downloaded, total: progress.total });
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [projectKey]);

  const sync = useCallback(
    async (release: Release, asset: ReleaseAsset) => {
      setState({ phase: "syncing", downloaded: 0, total: asset.size });
      try {
        await syncReleaseAsset(projectKey, release, asset);
        setState({ phase: "done" });
      } catch (error) {
        setState({ phase: "error", message: String(error) });
      }
    },
    [projectKey],
  );

  return { state, sync };
}
