import { useEffect, useState } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  getBuildDir,
  getBuildExecutable,
  getLatestBuildDir,
  getLatestBuildExecutable,
  launchBuild,
  launchLatestBuild,
} from "../api/sync";
import type { LatestChannel } from "../api/sync";

type Props =
  | {
      projectKey: string;
      releaseTag: string;
      latestChannel?: undefined;
      configName: string;
      disabled: boolean;
    }
  | {
      projectKey: string;
      releaseTag?: undefined;
      latestChannel: LatestChannel;
      configName: string;
      disabled: boolean;
    };

export function SyncedBuildControls(props: Props) {
  const { projectKey, configName, disabled } = props;
  const [buildDir, setBuildDir] = useState<string | null>(null);
  const [executable, setExecutable] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (props.latestChannel) {
      getLatestBuildDir(projectKey, props.latestChannel, configName)
        .then(setBuildDir)
        .catch((err) => console.error("failed to look up build dir", err));
      getLatestBuildExecutable(projectKey, props.latestChannel, configName)
        .then(setExecutable)
        .catch((err) => console.error("failed to look up build executable", err));
    } else {
      getBuildDir(projectKey, props.releaseTag, configName)
        .then(setBuildDir)
        .catch((err) => console.error("failed to look up build dir", err));
      getBuildExecutable(projectKey, props.releaseTag, configName)
        .then(setExecutable)
        .catch((err) => console.error("failed to look up build executable", err));
    }
  }, [projectKey, props.releaseTag, props.latestChannel, configName]);

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
      if (props.latestChannel) {
        await launchLatestBuild(projectKey, props.latestChannel, configName);
      } else {
        await launchBuild(projectKey, props.releaseTag, configName);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="synced-build-row">
      <span className="synced-build-row__name">{configName}</span>
      {buildDir && (
        <button aria-label={`Open folder for ${configName}`} disabled={disabled} onClick={handleOpenFolder}>
          Open Folder
        </button>
      )}
      {executable && (
        <button
          className="synced-build-row__launch"
          aria-label={`Launch ${configName}`}
          disabled={disabled}
          onClick={handleLaunch}
        >
          ▶ Launch
        </button>
      )}
      {error && <p className="error-text">{error}</p>}
    </div>
  );
}
