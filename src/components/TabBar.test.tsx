import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { fireEvent } from "@testing-library/react";
import { TabBar } from "./TabBar";
import type { Project } from "../api/projects";

const projects: Project[] = [
  { full_name: "org/repo-a", owner: "org", name: "repo-a", favorite: false },
  { full_name: "org/repo-b", owner: "org", name: "repo-b", favorite: false },
];

describe("TabBar", () => {
  it("shows only the bind button when nothing is bound", () => {
    render(
      <TabBar
        projects={projects}
        boundKeys={[]}
        activeKey={null}
        onSelect={() => {}}
        onUnbind={() => {}}
        onRequestBind={() => {}}
      />,
    );

    expect(screen.getByRole("button", { name: "Bind a project" })).toBeInTheDocument();
    expect(screen.queryByRole("tab")).not.toBeInTheDocument();
  });

  it("renders a tab per bound project, in bind order, using its display name", () => {
    render(
      <TabBar
        projects={projects}
        boundKeys={["org/repo-b", "org/repo-a"]}
        activeKey={null}
        onSelect={() => {}}
        onUnbind={() => {}}
        onRequestBind={() => {}}
      />,
    );

    const tabs = screen.getAllByRole("tab");
    expect(tabs).toHaveLength(2);
    expect(tabs[0]).toHaveTextContent("repo-b");
    expect(tabs[1]).toHaveTextContent("repo-a");
  });

  it("marks the active tab as selected", () => {
    render(
      <TabBar
        projects={projects}
        boundKeys={["org/repo-a", "org/repo-b"]}
        activeKey="org/repo-b"
        onSelect={() => {}}
        onUnbind={() => {}}
        onRequestBind={() => {}}
      />,
    );

    expect(screen.getByRole("tab", { selected: true })).toHaveTextContent("repo-b");
  });

  it("calls onSelect with the project key when a tab is clicked", () => {
    const onSelect = vi.fn();
    render(
      <TabBar
        projects={projects}
        boundKeys={["org/repo-a"]}
        activeKey={null}
        onSelect={onSelect}
        onUnbind={() => {}}
        onRequestBind={() => {}}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "repo-a" }));

    expect(onSelect).toHaveBeenCalledWith("org/repo-a");
  });

  it("calls onUnbind with the project key when a tab's close button is clicked", () => {
    const onUnbind = vi.fn();
    render(
      <TabBar
        projects={projects}
        boundKeys={["org/repo-a"]}
        activeKey={null}
        onSelect={() => {}}
        onUnbind={onUnbind}
        onRequestBind={() => {}}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Close repo-a" }));

    expect(onUnbind).toHaveBeenCalledWith("org/repo-a");
  });

  it("calls onRequestBind when the + button is clicked", () => {
    const onRequestBind = vi.fn();
    render(
      <TabBar
        projects={projects}
        boundKeys={[]}
        activeKey={null}
        onSelect={() => {}}
        onUnbind={() => {}}
        onRequestBind={onRequestBind}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Bind a project" }));

    expect(onRequestBind).toHaveBeenCalled();
  });
});
