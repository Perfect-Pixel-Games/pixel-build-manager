import type { SyncState } from "../hooks/useSync";

type Props = {
  state: SyncState;
};

export function SyncStatus({ state }: Props) {
  if (state.phase === "done") {
    return <p>Synced.</p>;
  }
  if (state.phase === "error") {
    return <p>Sync failed: {state.message}</p>;
  }
  return null;
}
