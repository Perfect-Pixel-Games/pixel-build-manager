import { useState, type FormEvent } from "react";
import type { Project } from "../api/projects";

type Props = {
  projects: Project[];
  /** Logged out, only public repos can be bound (by URL); there's no list to browse. */
  loggedIn: boolean;
  onBind: (fullName: string) => void;
  /** Binds a repo by its GitHub URL; rejects with a message to show. */
  onBindUrl: (url: string) => Promise<void>;
  onToggleFavorite: (fullName: string, favorite: boolean) => void;
  onClose: () => void;
};

export function BindProjectPopup({ projects, loggedIn, onBind, onBindUrl, onToggleFavorite, onClose }: Props) {
  const [search, setSearch] = useState("");
  const [url, setUrl] = useState("");
  const [urlError, setUrlError] = useState<string | null>(null);
  const [bindingUrl, setBindingUrl] = useState(false);
  const filtered = projects.filter((project) =>
    project.name.toLowerCase().includes(search.toLowerCase()),
  );

  const handleSubmitUrl = async (event: FormEvent) => {
    event.preventDefault();
    if (!url.trim() || bindingUrl) {
      return;
    }
    setBindingUrl(true);
    setUrlError(null);
    try {
      await onBindUrl(url.trim());
    } catch (error) {
      setUrlError(String(error));
    } finally {
      setBindingUrl(false);
    }
  };

  return (
    <div className="popup-backdrop" onClick={onClose}>
      <div
        className="popup"
        role="dialog"
        aria-label="Bind a project"
        onClick={(event) => event.stopPropagation()}
      >
        {loggedIn ? (
          <>
            <input
              className="popup__search"
              aria-label="Search projects"
              placeholder="Search projects..."
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
            <div className="popup__list">
              {filtered.length === 0 ? (
                <p className="release-list__empty">No matching projects.</p>
              ) : (
                filtered.map((project) => (
                  <div key={project.full_name} className="popup__row">
                    <button
                      className="popup__star"
                      aria-label={`Toggle favorite for ${project.name}`}
                      aria-pressed={project.favorite}
                      onClick={() => onToggleFavorite(project.full_name, !project.favorite)}
                    >
                      {project.favorite ? "★" : "☆"}
                    </button>
                    <button className="popup__project" onClick={() => onBind(project.full_name)}>
                      {project.name}
                    </button>
                  </div>
                ))
              )}
            </div>
          </>
        ) : (
          <p className="popup__hint">
            You're not logged in, so only public repositories can be bound. Log in to GitHub to browse
            your own and your organisations' projects.
          </p>
        )}
        <form className="popup__url" onSubmit={handleSubmitUrl}>
          <label className="popup__url-label" htmlFor="bind-repo-url">
            {loggedIn ? "Or bind any public repo by URL" : "Bind a public repo by URL"}
          </label>
          <div className="popup__url-row">
            <input
              id="bind-repo-url"
              placeholder="https://github.com/owner/repo"
              value={url}
              onChange={(event) => {
                setUrl(event.target.value);
                setUrlError(null);
              }}
              disabled={bindingUrl}
            />
            <button type="submit" disabled={bindingUrl || !url.trim()}>
              {bindingUrl ? "Checking..." : "Bind"}
            </button>
          </div>
          {urlError && (
            <p className="error-text" role="alert">
              {urlError}
            </p>
          )}
        </form>
        <button className="popup__close" onClick={onClose}>
          Close
        </button>
      </div>
    </div>
  );
}
