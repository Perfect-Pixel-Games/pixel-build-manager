import { useCallback, useEffect, useState, type ReactNode } from "react";
import { isLoggedIn, logout, SESSION_EXPIRED_ERROR } from "./api/auth";
import { listProjects, listReleasesForProject, toggleFavorite, Project, Release, ReleaseAsset } from "./api/projects";
import { bindProject, getWorkspaceRoot, listBoundProjects, unbindProject } from "./api/settings";
import {
  getSelectedRelease,
  getTickedConfigs,
  listSyncedConfigs,
  setSelectedRelease,
  setTickedConfigs,
} from "./api/sync";
import { getVersionLabel } from "./api/version";
import { BindProjectPopup } from "./components/BindProjectPopup";
import { BuildBrowser } from "./components/BuildBrowser";
import { BusyOverlay } from "./components/BusyOverlay";
import { ClearCacheButton } from "./components/ClearCacheButton";
import { Login } from "./components/Login";
import { SyncedBuildControls } from "./components/SyncedBuildControls";
import { SyncStatus } from "./components/SyncStatus";
import { TabBar } from "./components/TabBar";
import { ThemeToggle } from "./components/ThemeToggle";
import { WorkspaceSetup } from "./components/WorkspaceSetup";
import { useSync } from "./hooks/useSync";
import { useTheme } from "./hooks/useTheme";
import "./App.css";

export function ProjectDetail({ projectKey }: { projectKey: string }) {
  const [releases, setReleases] = useState<Release[]>([]);
  const [selectedReleaseTag, setSelectedReleaseTagState] = useState<string | null>(null);
  const [tickedConfigs, setTickedConfigsState] = useState<string[]>([]);
  const [syncedConfigs, setSyncedConfigs] = useState<string[]>([]);
  const { state, syncConfigs } = useSync(projectKey);

  useEffect(() => {
    listReleasesForProject(projectKey)
      .then(setReleases)
      .catch((error) => console.error("failed to load releases", error));
    getSelectedRelease(projectKey)
      .then(setSelectedReleaseTagState)
      .catch((error) => console.error("failed to load selected release", error));
    getTickedConfigs(projectKey)
      .then(setTickedConfigsState)
      .catch((error) => console.error("failed to load ticked configs", error));
  }, [projectKey]);

  useEffect(() => {
    if (!selectedReleaseTag) {
      setSyncedConfigs([]);
      return;
    }
    listSyncedConfigs(projectKey, selectedReleaseTag)
      .then(setSyncedConfigs)
      .catch((error) => console.error("failed to list synced configs", error));
  }, [projectKey, selectedReleaseTag]);

  const isBusy = state.phase === "syncing";

  const handleSelectRelease = (tag: string) => {
    setSelectedReleaseTagState(tag);
    setSelectedRelease(projectKey, tag).catch((error) =>
      console.error("failed to persist selected release", error),
    );
  };

  const handleToggleConfig = (configName: string, ticked: boolean) => {
    setTickedConfigsState((prev) => {
      const next = ticked ? [...prev, configName] : prev.filter((c) => c !== configName);
      setTickedConfigs(projectKey, next).catch((error) =>
        console.error("failed to persist ticked configs", error),
      );
      return next;
    });
  };

  const handleSync = async (release: Release, assets: ReleaseAsset[]) => {
    let succeeded = false;
    try {
      succeeded = await syncConfigs(release, assets);
    } catch (error) {
      console.error("failed to sync build configs", error);
      return;
    }
    if (succeeded && selectedReleaseTag === release.tag_name) {
      try {
        setSyncedConfigs(await listSyncedConfigs(projectKey, release.tag_name));
      } catch (error) {
        console.error("failed to refresh synced configs", error);
      }
    }
  };

  const busyLabel =
    state.phase === "syncing"
      ? state.downloaded >= state.total
        ? `Finishing up ${state.configName}...`
        : `Downloading ${state.configName}: ${Math.round((state.downloaded / state.total) * 100)}%`
      : undefined;

  return (
    <div>
      <BusyOverlay active={isBusy} label={busyLabel}>
        <BuildBrowser
          releases={releases}
          selectedReleaseTag={selectedReleaseTag}
          onSelectRelease={handleSelectRelease}
          tickedConfigs={tickedConfigs}
          onToggleConfig={handleToggleConfig}
          syncedConfigs={syncedConfigs}
          onSync={handleSync}
          disabled={isBusy}
        />
        <SyncStatus state={state} />
        {selectedReleaseTag &&
          syncedConfigs.map((config) => (
            <SyncedBuildControls
              key={config}
              projectKey={projectKey}
              releaseTag={selectedReleaseTag}
              configName={config}
              disabled={isBusy}
            />
          ))}
      </BusyOverlay>
      <ClearCacheButton
        projectKey={projectKey}
        onCleared={() => setSyncedConfigs([])}
        disabled={isBusy}
      />
    </div>
  );
}

function App() {
  const [loggedIn, setLoggedIn] = useState<boolean | null>(null);
  const [workspaceRoot, setWorkspaceRootState] = useState<string | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [boundKeys, setBoundKeys] = useState<string[]>([]);
  const [activeTab, setActiveTab] = useState<string | null>(null);
  const [showBindPopup, setShowBindPopup] = useState(false);
  const [versionLabel, setVersionLabel] = useState<string | null>(null);
  const { theme, setTheme } = useTheme();

  useEffect(() => {
    getVersionLabel()
      .then(setVersionLabel)
      .catch((error) => console.error("failed to load version label", error));
  }, []);

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
        .then((data) => {
          setProjects(data);
          listBoundProjects()
            .then(setBoundKeys)
            .catch((error) => console.error("failed to load bound projects", error));
        })
        .catch((error) => {
          if (error === SESSION_EXPIRED_ERROR) {
            setLoggedIn(false);
            return;
          }
          console.error("failed to load projects", error);
        });
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

  const handleBind = async (fullName: string) => {
    try {
      await bindProject(fullName);
      setBoundKeys((prev) => [...prev, fullName]);
      setActiveTab(fullName);
      setShowBindPopup(false);
    } catch (error) {
      console.error("failed to bind project", error);
    }
  };

  const handleUnbind = async (fullName: string) => {
    try {
      await unbindProject(fullName);
      setBoundKeys((prev) => prev.filter((key) => key !== fullName));
      setActiveTab((prev) => (prev === fullName ? null : prev));
    } catch (error) {
      console.error("failed to unbind project", error);
    }
  };

  const handleLoggedIn = useCallback(() => setLoggedIn(true), []);

  const handleLogout = async () => {
    try {
      await logout();
      setLoggedIn(false);
    } catch (error) {
      console.error("failed to log out", error);
    }
  };

  let content: ReactNode;
  if (loggedIn === null) {
    content = <p>Loading...</p>;
  } else if (!loggedIn) {
    content = <Login onLoggedIn={handleLoggedIn} />;
  } else if (!workspaceRoot) {
    content = <WorkspaceSetup onSet={setWorkspaceRootState} />;
  } else {
    content = (
      <div>
        <div className="top-bar">
          <TabBar
            projects={projects}
            boundKeys={boundKeys}
            activeKey={activeTab}
            onSelect={setActiveTab}
            onUnbind={handleUnbind}
            onRequestBind={() => setShowBindPopup(true)}
          />
          <ThemeToggle theme={theme} onChange={setTheme} />
          <button onClick={handleLogout}>Log out</button>
        </div>
        {showBindPopup && (
          <BindProjectPopup
            projects={projects.filter((p) => !boundKeys.includes(p.full_name))}
            onBind={handleBind}
            onToggleFavorite={handleToggleFavorite}
            onClose={() => setShowBindPopup(false)}
          />
        )}
        {activeTab && <ProjectDetail key={activeTab} projectKey={activeTab} />}
      </div>
    );
  }

  return (
    <div className="app-shell">
      <div className="app-content">{content}</div>
      <footer className="app-footer">{versionLabel}</footer>
    </div>
  );
}

export default App;
