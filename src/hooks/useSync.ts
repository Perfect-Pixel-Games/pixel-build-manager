import { useCallback, useEffect, useRef, useState } from "react";
import { onSyncProgress, syncReleaseAsset } from "../api/sync";
import type { Release, ReleaseAsset } from "../api/projects";

export type SyncState =
  | { phase: "idle" }
  | { phase: "syncing"; configName: string; downloaded: number; total: number }
  | { phase: "done" }
  | { phase: "error"; message: string };

/** Syncs (downloads + extracts) one or more build configs of a release, one
 * at a time, reporting progress for whichever config is currently
 * downloading. Guards against overlapping calls the same way the previous
 * single-asset version did. */
export function useSync(projectKey: string) {
  const [state, setState] = useState<SyncState>({ phase: "idle" });
  const inFlightRef = useRef(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    onSyncProgress((progress) => {
      if (progress.project_key !== projectKey) {
        return;
      }
      setState((prev) =>
        prev.phase === "syncing"
          ? { ...prev, downloaded: progress.downloaded, total: progress.total }
          : prev,
      );
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

  // Runs sync for every asset in sequence, guarding against overlapping
  // calls. Returns true if every asset synced successfully, false if it was
  // skipped because another sync was already in flight, and rethrows (after
  // recording the error state) on the first failure, stopping the batch --
  // so the caller never mistakes a partially-synced batch for a fully
  // completed one.
  const syncConfigs = useCallback(
    async (release: Release, assets: ReleaseAsset[]) => {
      if (inFlightRef.current) {
        return false;
      }
      inFlightRef.current = true;
      try {
        for (const asset of assets) {
          setState({ phase: "syncing", configName: asset.name, downloaded: 0, total: asset.size });
          await syncReleaseAsset(projectKey, release, asset);
        }
        setState({ phase: "done" });
        return true;
      } catch (error) {
        setState({ phase: "error", message: String(error) });
        throw error;
      } finally {
        inFlightRef.current = false;
      }
    },
    [projectKey],
  );

  return { state, syncConfigs };
}
