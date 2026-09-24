import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { BindProjectPopup } from "./BindProjectPopup";
import type { Project } from "../api/projects";

const projects: Project[] = [
  { full_name: "org/last-beacon", owner: "org", name: "last-beacon", favorite: true },
  { full_name: "org/other-game", owner: "org", name: "other-game", favorite: false },
];

describe("BindProjectPopup", () => {
  it("lists every candidate project", () => {
    render(
      <BindProjectPopup projects={projects} onBind={() => {}} onToggleFavorite={() => {}} onClose={() => {}} />,
    );

    expect(screen.getByRole("button", { name: "last-beacon" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "other-game" })).toBeInTheDocument();
  });

  it("filters the list by search text", () => {
    render(
      <BindProjectPopup projects={projects} onBind={() => {}} onToggleFavorite={() => {}} onClose={() => {}} />,
    );

    fireEvent.change(screen.getByLabelText("Search projects"), { target: { value: "other" } });

    expect(screen.queryByRole("button", { name: "last-beacon" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "other-game" })).toBeInTheDocument();
  });

  it("calls onBind with the project's full name when clicked", () => {
    const onBind = vi.fn();
    render(
      <BindProjectPopup projects={projects} onBind={onBind} onToggleFavorite={() => {}} onClose={() => {}} />,
    );

    fireEvent.click(screen.getByRole("button", { name: "last-beacon" }));

    expect(onBind).toHaveBeenCalledWith("org/last-beacon");
  });

  it("calls onToggleFavorite when a star is clicked", () => {
    const onToggleFavorite = vi.fn();
    render(
      <BindProjectPopup
        projects={projects}
        onBind={() => {}}
        onToggleFavorite={onToggleFavorite}
        onClose={() => {}}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Toggle favorite for other-game" }));

    expect(onToggleFavorite).toHaveBeenCalledWith("org/other-game", true);
  });

  it("calls onClose when Close is clicked", () => {
    const onClose = vi.fn();
    render(
      <BindProjectPopup projects={projects} onBind={() => {}} onToggleFavorite={() => {}} onClose={onClose} />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Close" }));

    expect(onClose).toHaveBeenCalled();
  });
});
