import { useCallback, useEffect, useState } from "react";
import { isLoggedIn, logout } from "./api/auth";
import { listProjects, listReleasesForProject, toggleFavorite, Project, Release, ReleaseAsset } from "./api/projects";
import { getWorkspaceRoot } from "./api/settings";
import { getActiveRelease } from "./api/sync";
import { Login } from "./components/Login";
import { ProjectList } from "./components/ProjectList";
import { ReleaseList } from "./components/ReleaseList";
import { SyncStatus } from "./components/SyncStatus";
import { WorkspaceSetup } from "./components/WorkspaceSetup";
import { useSync } from "./hooks/useSync";
import "./App.css";

function ProjectDetail({ projectKey }: { projectKey: string }) {
  const [releases, setReleases] = useState<Release[]>([]);
  const [activeAssetName, setActiveAssetName] = useState<string | null>(null);
  const { state, sync } = useSync(projectKey);

  useEffect(() => {
    listReleasesForProject(projectKey)
      .then(setReleases)
      .catch((error) => console.error("failed to load releases", error));
    getActiveRelease(projectKey)
      .then((active) => setActiveAssetName(active.asset_name))
      .catch((error) => console.error("failed to load active release", error));
  }, [projectKey]);

  const handleSync = async (release: Release, assetId: number) => {
    const asset = release.assets.find((a) => a.id === assetId) as ReleaseAsset;
    await sync(release, asset);
    setActiveAssetName(asset.name);
  };

  return (
    <div>
      <SyncStatus state={state} />
      <ReleaseList releases={releases} activeAssetName={activeAssetName} onSync={handleSync} />
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
    await toggleFavorite(fullName, favorite);
    setProjects((prev) => prev.map((p) => (p.full_name === fullName ? { ...p, favorite } : p)));
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
          await logout();
          setLoggedIn(false);
        }}
      >
        Log out
      </button>
      <ProjectList projects={projects} onSelect={setSelectedProject} onToggleFavorite={handleToggleFavorite} />
      {selectedProject && <ProjectDetail projectKey={selectedProject} />}
    </div>
  );
}

export default App;
