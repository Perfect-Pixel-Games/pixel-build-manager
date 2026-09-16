import { useCallback, useEffect, useRef, useState } from "react";
import { checkReleaseAsset, onSyncProgress, syncReleaseAsset } from "../api/sync";
import type { Release, ReleaseAsset } from "../api/projects";

export type SyncState =
  | { phase: "idle" }
  | { phase: "syncing"; downloaded: number; total: number }
  | { phase: "done" }
  | { phase: "error"; message: string };

export function useSync(projectKey: string) {
  const [state, setState] = useState<SyncState>({ phase: "idle" });
  // Tracked outside React state so `runExclusive` can check it without
  // depending on `state` (which would break its referential stability).
  // Guards against overlapping sync()/check() calls racing to write state
  // independently -- see the Task 23 review for the double-click scenario
  // this prevents.
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

  // Runs `action` while guarding against overlapping calls. Returns `true`
  // if `action` ran and succeeded, `false` if it was skipped because another
  // call was already in flight, and rethrows (after recording the error
  // state) if `action` failed -- callers use the return value to decide
  // whether it's safe to treat the operation as having actually completed
  // (e.g. only marking a build active after a real success).
  const runExclusive = useCallback(async (asset: ReleaseAsset, action: () => Promise<void>) => {
    if (inFlightRef.current) {
      return false;
    }
    inFlightRef.current = true;
    setState({ phase: "syncing", downloaded: 0, total: asset.size });
    try {
      await action();
      setState({ phase: "done" });
      return true;
    } catch (error) {
      setState({ phase: "error", message: String(error) });
      throw error;
    } finally {
      inFlightRef.current = false;
    }
  }, []);

  const sync = useCallback(
    (release: Release, asset: ReleaseAsset) =>
      runExclusive(asset, () => syncReleaseAsset(projectKey, release, asset)),
    [projectKey, runExclusive],
  );

  const check = useCallback(
    (asset: ReleaseAsset) => runExclusive(asset, () => checkReleaseAsset(projectKey, asset)),
    [projectKey, runExclusive],
  );

  return { state, sync, check };
}
