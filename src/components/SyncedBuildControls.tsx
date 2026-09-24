import { useEffect, useState } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import { getBuildDir, getBuildExecutable, launchBuild } from "../api/sync";

type Props = {
  projectKey: string;
  releaseTag: string;
  configName: string;
  disabled: boolean;
};

export function SyncedBuildControls({ projectKey, releaseTag, configName, disabled }: Props) {
  const [buildDir, setBuildDir] = useState<string | null>(null);
  const [executable, setExecutable] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getBuildDir(projectKey, releaseTag, configName)
      .then(setBuildDir)
      .catch((err) => console.error("failed to look up build dir", err));
    getBuildExecutable(projectKey, releaseTag, configName)
      .then(setExecutable)
      .catch((err) => console.error("failed to look up build executable", err));
  }, [projectKey, releaseTag, configName]);

  const handleOpenFolder = async () => {
    setError(null);
    if (!buildDir) {
      return;
    }
    try {
      await openPath(buildDir);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleLaunch = async () => {
    setError(null);
    try {
      await launchBuild(projectKey, releaseTag, configName);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <span>
      {buildDir && (
        <button aria-label={`Open folder for ${configName}`} disabled={disabled} onClick={handleOpenFolder}>
          Open Folder
        </button>
      )}
      {executable && (
        <button aria-label={`Launch ${configName}`} disabled={disabled} onClick={handleLaunch}>
          Launch
        </button>
      )}
      {error && <p>{error}</p>}
    </span>
  );
}
