import { useCallback, useEffect, useState } from "react";
import { isLoggedIn, logout } from "./api/auth";
import { listProjects, listReleasesForProject, toggleFavorite, Project, Release } from "./api/projects";
import { Login } from "./components/Login";
import { ProjectList } from "./components/ProjectList";
import { ReleaseList } from "./components/ReleaseList";
import "./App.css";

function App() {
  const [loggedIn, setLoggedIn] = useState<boolean | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProject, setSelectedProject] = useState<string | null>(null);
  const [releases, setReleases] = useState<Release[]>([]);

  useEffect(() => {
    isLoggedIn()
      .then(setLoggedIn)
      .catch(() => setLoggedIn(false));
  }, []);

  useEffect(() => {
    if (loggedIn) {
      listProjects()
        .then(setProjects)
        .catch((error) => console.error("failed to load projects", error));
    }
  }, [loggedIn]);

  useEffect(() => {
    if (selectedProject) {
      listReleasesForProject(selectedProject)
        .then(setReleases)
        .catch((error) => console.error("failed to load releases", error));
    }
  }, [selectedProject]);

  const handleToggleFavorite = async (fullName: string, favorite: boolean) => {
    await toggleFavorite(fullName, favorite);
    setProjects((prev) =>
      prev.map((p) => (p.full_name === fullName ? { ...p, favorite } : p)),
    );
  };

  const handleLoggedIn = useCallback(() => setLoggedIn(true), []);

  if (loggedIn === null) {
    return <p>Loading...</p>;
  }

  if (!loggedIn) {
    return <Login onLoggedIn={handleLoggedIn} />;
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
      <ProjectList
        projects={projects}
        onSelect={setSelectedProject}
        onToggleFavorite={handleToggleFavorite}
      />
      {selectedProject && (
        <ReleaseList releases={releases} activeAssetName={null} onSync={() => {}} />
      )}
    </div>
  );
}

export default App;
