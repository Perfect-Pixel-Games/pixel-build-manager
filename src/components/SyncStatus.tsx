import type { SyncState } from "../hooks/useSync";

type Props = {
  state: SyncState;
};

export function SyncStatus({ state }: Props) {
  if (state.phase === "idle") {
    return null;
  }

  if (state.phase === "syncing") {
    // Assumes state.total > 0 for any real asset; a zero-byte total would
    // satisfy `downloaded >= total` immediately and show "Finishing up..."
    // instead of a percentage.
    if (state.downloaded >= state.total) {
      return <p>Finishing up...</p>;
    }
    const percent = Math.round((state.downloaded / state.total) * 100);
    return <p>Downloading: {percent}%</p>;
  }

  if (state.phase === "done") {
    return <p>Synced.</p>;
  }

  return <p>Sync failed: {state.message}</p>;
}
