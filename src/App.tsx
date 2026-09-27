import { useCallback, useEffect, useState, type ReactNode } from "react";
import {
  confirmLogout,
  isLoggedIn,
  listAccountBoundProjects,
  logout,
  SESSION_EXPIRED_ERROR,
} from "./api/auth";
import { listProjects, listReleasesForProject, toggleFavorite, Project, Release, ReleaseAsset } from "./api/projects";
import { bindProject, bindProjectByUrl, getWorkspaceRoot, listBoundProjects, unbindProject } from "./api/settings";
import {
  checkLatestNow,
  getSelectedRelease,
  getSyncMode,
  getTickedConfigs,
  listSyncedConfigs,
  listSyncedLatestConfigs,
  setSelectedRelease,
  setSyncMode,
  setTickedConfigs,
  type LatestChannel,
  type SyncMode,
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
import { useLatestSync } from "./hooks/useLatestSync";
import { useSync } from "./hooks/useSync";
import { useTheme } from "./hooks/useTheme";
import "./App.css";

export function ProjectDetail({ projectKey, loggedIn = true }: { projectKey: string; loggedIn?: boolean }) {
  const [releases, setReleases] = useState<Release[]>([]);
  const [releasesFailed, setReleasesFailed] = useState(false);
  const [selectedReleaseTag, setSelectedReleaseTagState] = useState<string | null>(null);
  const [tickedConfigs, setTickedConfigsState] = useState<string[]>([]);
  const [syncedConfigs, setSyncedConfigs] = useState<string[]>([]);
  const [syncMode, setSyncModeState] = useState<SyncMode>("manual");
  const { state, syncConfigs } = useSync(projectKey);
  const { state: latestState } = useLatestSync(projectKey);

  useEffect(() => {
    listReleasesForProject(projectKey)
      .then(setReleases)
      .catch((error) => {
        console.error("failed to load releases", error);
        setReleasesFailed(true);
      });
    getSelectedRelease(projectKey)
      .then(setSelectedReleaseTagState)
      .catch((error) => console.error("failed to load selected release", error));
    getTickedConfigs(projectKey)
      .then(setTickedConfigsState)
      .catch((error) => console.error("failed to load ticked configs", error));
    getSyncMode(projectKey)
      .then(setSyncModeState)
      .catch((error) => console.error("failed to load sync mode", error));
  }, [projectKey]);

  useEffect(() => {
    let cancelled = false;

    if (syncMode === "manual") {
      if (!selectedReleaseTag) {
        setSyncedConfigs([]);
        return;
      }
      listSyncedConfigs(projectKey, selectedReleaseTag)
        .then((configs) => {
          if (!cancelled) {
            setSyncedConfigs(configs);
          }
        })
        .catch((error) => console.error("failed to list synced configs", error));
      return () => {
        cancelled = true;
      };
    }

    const channel: LatestChannel = syncMode === "latest_prerelease" ? "prerelease" : "release";
    listSyncedLatestConfigs(projectKey, channel)
      .then((configs) => {
        if (!cancelled) {
          setSyncedConfigs(configs);
        }
      })
      .catch((error) => console.error("failed to list synced latest configs", error));
    return () => {
      cancelled = true;
    };
  }, [projectKey, syncMode, selectedReleaseTag, latestState.phase]);

  const isManualSyncing = state.phase === "syncing";
  const isLatestSyncing = latestState.phase === "syncing";
  const isBusy = isManualSyncing || isLatestSyncing;

  const handleSelectRelease = (tag: string) => {
    setSyncModeState("manual");
    setSelectedReleaseTagState(tag);
    setSyncMode(projectKey, "manual").catch((error) => console.error("failed to persist sync mode", error));
    setSelectedRelease(projectKey, tag).catch((error) =>
      console.error("failed to persist selected release", error),
    );
  };

  const handleSelectLatest = (mode: "latest_release" | "latest_prerelease") => {
    setSyncModeState(mode);
    setSyncMode(projectKey, mode).catch((error) => console.error("failed to persist sync mode", error));
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

  const handleSyncLatest = () => {
    checkLatestNow(projectKey).catch((error) => console.error("failed to trigger latest sync", error));
  };

  const busyLabel = isManualSyncing
    ? state.downloaded >= state.total
      ? `Finishing up ${state.configName}...`
      : `Downloading ${state.configName}: ${Math.round((state.downloaded / state.total) * 100)}%`
    : isLatestSyncing
      ? `Syncing latest ${latestState.channel}: ${Math.round((latestState.downloaded / latestState.total) * 100)}%`
      : undefined;

  const showSyncedBuilds = (syncMode !== "manual" || selectedReleaseTag !== null) && syncedConfigs.length > 0;

  return (
    <div className="project-detail">
      {releasesFailed && (
        <p className="error-text" role="alert">
          Couldn't load releases for {projectKey}.
          {!loggedIn && " If it's a private repository, log in to GitHub to access it."}
        </p>
      )}
      <BusyOverlay active={isBusy} label={busyLabel}>
        <BuildBrowser
          releases={releases}
          syncMode={syncMode}
          selectedReleaseTag={selectedReleaseTag}
          onSelectRelease={handleSelectRelease}
          onSelectLatest={handleSelectLatest}
          tickedConfigs={tickedConfigs}
          onToggleConfig={handleToggleConfig}
          syncedConfigs={syncedConfigs}
          onSync={handleSync}
          onSyncLatest={handleSyncLatest}
          disabled={isBusy}
        />
        <SyncStatus state={state} />
        {showSyncedBuilds && (
          <div className="synced-builds">
            {syncedConfigs.map((config) =>
              syncMode === "manual" ? (
                <SyncedBuildControls
                  key={config}
                  projectKey={projectKey}
                  releaseTag={selectedReleaseTag as string}
                  configName={config}
                  disabled={isBusy}
                />
              ) : (
                <SyncedBuildControls
                  key={config}
                  projectKey={projectKey}
                  latestChannel={syncMode === "latest_prerelease" ? "prerelease" : "release"}
                  configName={config}
                  disabled={isBusy}
                />
              ),
            )}
          </div>
        )}
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
  // `undefined` until loaded, so the workspace-setup screen doesn't flash.
  const [workspaceRoot, setWorkspaceRootState] = useState<string | null | undefined>(undefined);
  const [showLogin, setShowLogin] = useState(false);
  const [logoutError, setLogoutError] = useState<string | null>(null);
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

  // Logging in is optional, so the workspace and bound tabs load regardless.
  useEffect(() => {
    getWorkspaceRoot()
      .then(setWorkspaceRootState)
      .catch((error) => {
        console.error("failed to load workspace root", error);
        setWorkspaceRootState(null);
      });
    listBoundProjects()
      .then((keys) => {
        setBoundKeys(keys);
        // Land on the first bound tab by default, rather than an empty
        // pane, on every fresh load -- but never override a tab the user
        // already picked this session.
        setActiveTab((prev) => prev ?? keys[0] ?? null);
      })
      .catch((error) => console.error("failed to load bound projects", error));
  }, []);

  // The browsable project list only exists when logged in; logged out,
  // repos can only be bound by URL.
  useEffect(() => {
    if (!loggedIn) {
      setProjects([]);
      return;
    }
    listProjects()
      .then(setProjects)
      .catch((error) => {
        if (error === SESSION_EXPIRED_ERROR) {
          setLoggedIn(false);
          return;
        }
        console.error("failed to load projects", error);
      });
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
      setBoundKeys((prev) => (prev.includes(fullName) ? prev : [...prev, fullName]));
      setActiveTab(fullName);
      setShowBindPopup(false);
    } catch (error) {
      console.error("failed to bind project", error);
    }
  };

  // Errors propagate so the popup can show why the URL couldn't be bound.
  const handleBindUrl = async (url: string) => {
    const project = await bindProjectByUrl(url);
    setProjects((prev) =>
      prev.some((p) => p.full_name === project.full_name) ? prev : [...prev, project],
    );
    setBoundKeys((prev) => (prev.includes(project.full_name) ? prev : [...prev, project.full_name]));
    setActiveTab(project.full_name);
    setShowBindPopup(false);
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

  const handleLoggedIn = useCallback(() => {
    setLoggedIn(true);
    setShowLogin(false);
  }, []);

  const handleLogout = async () => {
    setLogoutError(null);
    try {
      const toRemove = await listAccountBoundProjects();
      if (toRemove.length > 0 && !(await confirmLogout(toRemove))) {
        return;
      }
      const removed = await logout();
      setBoundKeys((prev) => prev.filter((key) => !removed.includes(key)));
      setActiveTab((prev) => (prev !== null && removed.includes(prev) ? null : prev));
      setLoggedIn(false);
    } catch (error) {
      console.error("failed to log out", error);
      setLogoutError(String(error));
      // A partial failure may still have logged out and unbound projects,
      // so re-read the real state rather than guessing.
      isLoggedIn()
        .then(setLoggedIn)
        .catch(() => setLoggedIn(false));
      listBoundProjects()
        .then((keys) => {
          setBoundKeys(keys);
          setActiveTab((prev) => (prev !== null && keys.includes(prev) ? prev : null));
        })
        .catch((e) => console.error("failed to reload bound projects", e));
    }
  };

  let content: ReactNode;
  if (loggedIn === null || workspaceRoot === undefined) {
    content = (
      <div className="centered-screen">
        <p className="centered-card__lede">Loading...</p>
      </div>
    );
  } else if (showLogin) {
    content = <Login onLoggedIn={handleLoggedIn} onCancel={() => setShowLogin(false)} />;
  } else if (!workspaceRoot) {
    content = <WorkspaceSetup onSet={setWorkspaceRootState} />;
  } else {
    content = (
      <div className="main-view">
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
          {loggedIn ? (
            <button className="text-button" onClick={handleLogout}>
              Log out
            </button>
          ) : (
            <button className="text-button" title="Log in to GitHub" onClick={() => setShowLogin(true)}>
              Log in
            </button>
          )}
        </div>
        {logoutError && (
          <p className="error-text" role="alert">
            {logoutError}
          </p>
        )}
        {showBindPopup && (
          <BindProjectPopup
            projects={projects.filter((p) => !boundKeys.includes(p.full_name))}
            loggedIn={loggedIn}
            onBind={handleBind}
            onBindUrl={handleBindUrl}
            onToggleFavorite={handleToggleFavorite}
            onClose={() => setShowBindPopup(false)}
          />
        )}
        {activeTab ? (
          // Keyed on login state too, so releases reload with (or without)
          // credentials after logging in or out.
          <ProjectDetail key={`${activeTab}:${loggedIn}`} projectKey={activeTab} loggedIn={loggedIn} />
        ) : (
          <p className="empty-state">Bind a project to get started.</p>
        )}
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
