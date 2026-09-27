import { useEffect, useState } from "react";
import { onLatestSyncFinished, onLatestSyncProgress } from "../api/sync";
import type { LatestChannel } from "../api/sync";

export type LatestSyncState =
  | { phase: "idle" }
  | { phase: "syncing"; channel: LatestChannel; downloaded: number; total: number }
  | { phase: "done"; channel: LatestChannel; synced: boolean; tag: string | null }
  | { phase: "error"; channel: LatestChannel; message: string };

/** Tracks backend-initiated latest-release/prerelease syncs (background poll
 * or an immediate on-toggle check) for `projectKey`, via the
 * `latest-sync-progress`/`latest-sync-finished` events the Rust side emits.
 * Unlike `useSync`, nothing here is triggered by a call the frontend makes
 * directly -- the backend runs the whole cycle on its own, so this hook only
 * listens. */
export function useLatestSync(projectKey: string) {
  const [state, setState] = useState<LatestSyncState>({ phase: "idle" });

  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenFinished: (() => void) | undefined;
    let cancelled = false;

    onLatestSyncProgress((progress) => {
      if (progress.project_key !== projectKey) {
        return;
      }
      setState({
        phase: "syncing",
        channel: progress.channel,
        downloaded: progress.downloaded,
        total: progress.total,
      });
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlistenProgress = fn;
      }
    });

    onLatestSyncFinished((result) => {
      if (result.project_key !== projectKey) {
        return;
      }
      if (result.error) {
        setState({ phase: "error", channel: result.channel, message: result.error });
      } else {
        setState({ phase: "done", channel: result.channel, synced: result.synced, tag: result.tag });
      }
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlistenFinished = fn;
      }
    });

    return () => {
      cancelled = true;
      unlistenProgress?.();
      unlistenFinished?.();
    };
  }, [projectKey]);

  return { state };
}
