import type { Project } from "../api/projects";

type Props = {
  projects: Project[];
  onSelect: (fullName: string) => void;
  onToggleFavorite: (fullName: string, favorite: boolean) => void;
};

function ProjectRow({
  project,
  onSelect,
  onToggleFavorite,
}: {
  project: Project;
  onSelect: (fullName: string) => void;
  onToggleFavorite: (fullName: string, favorite: boolean) => void;
}) {
  return (
    <li>
      <button aria-label="toggle favorite" onClick={() => onToggleFavorite(project.full_name, !project.favorite)}>
        {project.favorite ? "★" : "☆"}
      </button>
      <button onClick={() => onSelect(project.full_name)}>{project.name}</button>
    </li>
  );
}

export function ProjectList({ projects, onSelect, onToggleFavorite }: Props) {
  const favorites = projects.filter((p) => p.favorite);

  return (
    <div>
      <section aria-label="Favorites">
        <h2>Favorites</h2>
        <ul>
          {favorites.map((project) => (
            <ProjectRow
              key={project.full_name}
              project={project}
              onSelect={onSelect}
              onToggleFavorite={onToggleFavorite}
            />
          ))}
        </ul>
      </section>
      <section aria-label="All projects">
        <h2>All projects</h2>
        <ul>
          {projects.map((project) => (
            <ProjectRow
              key={project.full_name}
              project={project}
              onSelect={onSelect}
              onToggleFavorite={onToggleFavorite}
            />
          ))}
        </ul>
      </section>
    </div>
  );
}
