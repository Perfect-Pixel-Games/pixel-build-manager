import { useCallback, useEffect, useState } from "react";
import { isLoggedIn, logout } from "./api/auth";
import { listProjects, listReleasesForProject, toggleFavorite, Project, Release, ReleaseAsset } from "./api/projects";
import { getWorkspaceRoot } from "./api/settings";
import {
  deleteCachedAsset,
  getActiveExecutable,
  getActiveRelease,
  launchActiveBuild,
  listCachedAssets,
} from "./api/sync";
import { ClearCacheButton } from "./components/ClearCacheButton";
import { Login } from "./components/Login";
import { ProjectList } from "./components/ProjectList";
import { ReleaseList } from "./components/ReleaseList";
import { SyncStatus } from "./components/SyncStatus";
import { WorkspaceSetup } from "./components/WorkspaceSetup";
import { useSync } from "./hooks/useSync";
import "./App.css";

function ProjectDetail({ projectKey }: { projectKey: string }) {
  const [releases, setReleases] = useState<Release[]>([]);
  const [activeReleaseTag, setActiveReleaseTag] = useState<string | null>(null);
  const [activeAssetName, setActiveAssetName] = useState<string | null>(null);
  const [activeExecutable, setActiveExecutable] = useState<string | null>(null);
  const [cachedAssetIds, setCachedAssetIds] = useState<Set<number>>(new Set());
  const [launchError, setLaunchError] = useState<string | null>(null);
  const { state, sync, check } = useSync(projectKey);

  useEffect(() => {
    listReleasesForProject(projectKey)
      .then(setReleases)
      .catch((error) => console.error("failed to load releases", error));
    getActiveRelease(projectKey)
      .then((active) => {
        setActiveReleaseTag(active.release_tag);
        setActiveAssetName(active.asset_name);
      })
      .catch((error) => console.error("failed to load active release", error));
    getActiveExecutable(projectKey)
      .then(setActiveExecutable)
      .catch((error) => console.error("failed to look up active executable", error));
    listCachedAssets(projectKey)
      .then((ids) => setCachedAssetIds(new Set(ids)))
      .catch((error) => console.error("failed to list cached assets", error));
  }, [projectKey]);

  // Picking an option activates it: download/verify, then extract as the
  // active build. Only mark it active if the sync actually completed --
  // `sync()` resolves false (without throwing) if it was skipped because
  // another operation was already in flight.
  const handleSelect = async (release: Release, assetId: number) => {
    const asset = release.assets.find((a) => a.id === assetId) as ReleaseAsset;
    let succeeded = false;
    try {
      succeeded = await sync(release, asset);
    } catch (error) {
      console.error("failed to sync/activate build", error);
      return;
    }
    if (!succeeded) {
      return;
    }
    setActiveReleaseTag(release.tag_name);
    setActiveAssetName(asset.name);
    setCachedAssetIds((prev) => new Set(prev).add(assetId));
    try {
      setActiveExecutable(await getActiveExecutable(projectKey));
    } catch (error) {
      console.error("failed to look up active executable", error);
    }
  };

  // Sync/Check only ensures a valid cached copy exists -- it never touches
  // the active build.
  const handleCheck = async (release: Release, assetId: number) => {
    const asset = release.assets.find((a) => a.id === assetId) as ReleaseAsset;
    try {
      const succeeded = await check(asset);
      if (succeeded) {
        setCachedAssetIds((prev) => new Set(prev).add(assetId));
      }
    } catch (error) {
      console.error("failed to check/download build", error);
    }
  };

  const handleDelete = async (release: Release, assetId: number) => {
    const asset = release.assets.find((a) => a.id === assetId) as ReleaseAsset;
    try {
      await deleteCachedAsset(projectKey, assetId, asset.name);
      setCachedAssetIds((prev) => {
        const next = new Set(prev);
        next.delete(assetId);
        return next;
      });
    } catch (error) {
      console.error("failed to delete cached asset", error);
    }
  };

  const handlePlay = async () => {
    setLaunchError(null);
    try {
      await launchActiveBuild(projectKey);
    } catch (error) {
      setLaunchError(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <div>
      <SyncStatus state={state} />
      {activeExecutable && (
        <div>
          <button onClick={handlePlay}>Play</button>
          {launchError && <p>Failed to launch: {launchError}</p>}
        </div>
      )}
      <ReleaseList
        releases={releases}
        activeReleaseTag={activeReleaseTag}
        activeAssetName={activeAssetName}
        cachedAssetIds={cachedAssetIds}
        onSelect={handleSelect}
        onCheck={handleCheck}
        onDelete={handleDelete}
      />
      <ClearCacheButton projectKey={projectKey} onCleared={() => setCachedAssetIds(new Set())} />
    </div>
  );
}

function App() {
  const [loggedIn, setLoggedIn] = useState<boolean | null>(null);
  const [workspaceRoot, setWorkspaceRootState] = useState<string | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProject, setSelectedProject] = useState<string | null>(null);

  useEffect(() => {
    isLoggedIn()
      .then(setLoggedIn)
      .catch(() => setLoggedIn(false));
  }, []);

  useEffect(() => {
    if (loggedIn) {
      getWorkspaceRoot()
        .then(setWorkspaceRootState)
        .catch((error) => console.error("failed to load workspace root", error));
      listProjects()
        .then(setProjects)
        .catch((error) => console.error("failed to load projects", error));
    }
  }, [loggedIn]);

  const handleToggleFavorite = async (fullName: string, favorite: boolean) => {
    try {
      await toggleFavorite(fullName, favorite);
      setProjects((prev) => prev.map((p) => (p.full_name === fullName ? { ...p, favorite } : p)));
    } catch (error) {
      console.error("failed to toggle favorite", error);
    }
  };

  const handleLoggedIn = useCallback(() => setLoggedIn(true), []);

  if (loggedIn === null) {
    return <p>Loading...</p>;
  }

  if (!loggedIn) {
    return <Login onLoggedIn={handleLoggedIn} />;
  }

  if (!workspaceRoot) {
    return <WorkspaceSetup onSet={setWorkspaceRootState} />;
  }

  return (
    <div>
      <button
        onClick={async () => {
          try {
            await logout();
            setLoggedIn(false);
          } catch (error) {
            console.error("failed to log out", error);
          }
        }}
      >
        Log out
      </button>
      <ProjectList projects={projects} onSelect={setSelectedProject} onToggleFavorite={handleToggleFavorite} />
      {selectedProject && <ProjectDetail key={selectedProject} projectKey={selectedProject} />}
    </div>
  );
}

export default App;
