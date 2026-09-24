import { useState } from "react";
import type { Project } from "../api/projects";

type Props = {
  projects: Project[];
  onBind: (fullName: string) => void;
  onToggleFavorite: (fullName: string, favorite: boolean) => void;
  onClose: () => void;
};

export function BindProjectPopup({ projects, onBind, onToggleFavorite, onClose }: Props) {
  const [search, setSearch] = useState("");
  const filtered = projects.filter((project) =>
    project.name.toLowerCase().includes(search.toLowerCase()),
  );

  return (
    <div className="popup-backdrop" onClick={onClose}>
      <div
        className="popup"
        role="dialog"
        aria-label="Bind a project"
        onClick={(event) => event.stopPropagation()}
      >
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
        <button className="popup__close" onClick={onClose}>
          Close
        </button>
      </div>
    </div>
  );
}
