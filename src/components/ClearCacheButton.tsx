import { useState } from "react";
import { clearProjectCache } from "../api/sync";

type Props = {
  projectKey: string;
};

export function ClearCacheButton({ projectKey }: Props) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleClick = async () => {
    setError(null);
    setPending(true);
    try {
      await clearProjectCache(projectKey);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setPending(false);
    }
  };

  return (
    <div>
      <button onClick={handleClick} disabled={pending}>
        {pending ? "Clearing cache..." : "Clear cache"}
      </button>
      {error && <p>Failed to clear cache: {error}</p>}
    </div>
  );
}
