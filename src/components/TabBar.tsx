import type { Project } from "../api/projects";

type Props = {
  projects: Project[];
  boundKeys: string[];
  activeKey: string | null;
  onSelect: (fullName: string) => void;
  onUnbind: (fullName: string) => void;
  onRequestBind: () => void;
};

export function TabBar({ projects, boundKeys, activeKey, onSelect, onUnbind, onRequestBind }: Props) {
  const byKey = new Map(projects.map((p) => [p.full_name, p]));

  return (
    <div className="tab-bar" role="tablist" aria-label="Bound projects">
      {boundKeys.map((key) => {
        // Repos bound by URL aren't in the user's accessible-project list,
        // so fall back to the repo segment of their `owner/repo` key.
        const name = byKey.get(key)?.name ?? key.split("/").pop() ?? key;
        return (
          <span key={key} className="tab" role="tab" aria-selected={key === activeKey}>
            <button onClick={() => onSelect(key)} title={key}>
              {name}
            </button>
            <button className="tab-close" aria-label={`Close ${name}`} onClick={() => onUnbind(key)}>
              ×
            </button>
          </span>
        );
      })}
      <button className="tab-bind" aria-label="Bind a project" onClick={onRequestBind}>
        +
      </button>
    </div>
  );
}
