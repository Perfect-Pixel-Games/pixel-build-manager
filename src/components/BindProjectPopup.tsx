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
    <div role="dialog" aria-label="Bind a project">
      <input
        aria-label="Search projects"
        value={search}
        onChange={(event) => setSearch(event.target.value)}
      />
      <ul>
        {filtered.map((project) => (
          <li key={project.full_name}>
            <button
              aria-label={`Toggle favorite for ${project.name}`}
              aria-pressed={project.favorite}
              onClick={() => onToggleFavorite(project.full_name, !project.favorite)}
            >
              {project.favorite ? "★" : "☆"}
            </button>
            <button onClick={() => onBind(project.full_name)}>{project.name}</button>
          </li>
        ))}
      </ul>
      <button onClick={onClose}>Close</button>
    </div>
  );
}
