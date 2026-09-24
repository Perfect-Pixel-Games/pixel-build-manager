import { useState } from "react";
import { clearProjectCache } from "../api/sync";

type Props = {
  projectKey: string;
  onCleared?: () => void;
  /** True while a download/check is in flight elsewhere -- clearing the cache mid-operation isn't safe. */
  disabled?: boolean;
};

export function ClearCacheButton({ projectKey, onCleared, disabled = false }: Props) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleClick = async () => {
    setError(null);
    setPending(true);
    try {
      await clearProjectCache(projectKey);
      onCleared?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setPending(false);
    }
  };

  return (
    <div className="clear-cache">
      <button onClick={handleClick} disabled={pending || disabled}>
        {pending ? "Clearing cache..." : "Clear cache"}
      </button>
      {error && <p className="error-text">Failed to clear cache: {error}</p>}
    </div>
  );
}
