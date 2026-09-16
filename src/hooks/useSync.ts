import { useCallback, useEffect, useRef, useState } from "react";
import { onSyncProgress, syncReleaseAsset } from "../api/sync";
import type { Release, ReleaseAsset } from "../api/projects";

export type SyncState =
  | { phase: "idle" }
  | { phase: "syncing"; downloaded: number; total: number }
  | { phase: "done" }
  | { phase: "error"; message: string };

export function useSync(projectKey: string) {
  const [state, setState] = useState<SyncState>({ phase: "idle" });
  // Tracked outside React state so `sync` can check it without depending on
  // `state` (which would break its referential stability). Guards against
  // overlapping sync() calls racing to write state independently -- see the
  // Task 23 review for the double-click scenario this prevents.
  const inFlightRef = useRef(false);

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
      if (inFlightRef.current) {
        return;
      }
      inFlightRef.current = true;
      setState({ phase: "syncing", downloaded: 0, total: asset.size });
      try {
        await syncReleaseAsset(projectKey, release, asset);
        setState({ phase: "done" });
      } catch (error) {
        setState({ phase: "error", message: String(error) });
      } finally {
        inFlightRef.current = false;
      }
    },
    [projectKey],
  );

  return { state, sync };
}
