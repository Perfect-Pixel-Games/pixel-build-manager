import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, within } from "@testing-library/react";
import { ProjectList } from "./ProjectList";
import type { Project } from "../api/projects";

const projects: Project[] = [
  { full_name: "pixel-perfect/last-beacon", owner: "pixel-perfect", name: "last-beacon", favorite: true },
  { full_name: "pixel-perfect/other-game", owner: "pixel-perfect", name: "other-game", favorite: false },
];

describe("ProjectList", () => {
  it("renders favorites in their own section, above the full list", () => {
    render(<ProjectList projects={projects} onSelect={() => {}} onToggleFavorite={() => {}} />);

    const favoritesSection = screen.getByRole("region", { name: /favorites/i });
    expect(favoritesSection).toHaveTextContent("last-beacon");

    const allSection = screen.getByRole("region", { name: /all projects/i });
    expect(allSection).toHaveTextContent("last-beacon");
    expect(allSection).toHaveTextContent("other-game");
  });

  it("calls onToggleFavorite when the star is clicked in the all-projects section", () => {
    const onToggleFavorite = vi.fn();
    render(<ProjectList projects={projects} onSelect={() => {}} onToggleFavorite={onToggleFavorite} />);

    const allSection = screen.getByRole("region", { name: /all projects/i });
    const otherGameRow = within(allSection).getByText("other-game").closest("li")!;
    fireEvent.click(within(otherGameRow).getByRole("button", { name: /toggle favorite/i }));

    expect(onToggleFavorite).toHaveBeenCalledWith("pixel-perfect/other-game", true);
  });

  it("calls onToggleFavorite when the star is clicked in the favorites section", () => {
    const onToggleFavorite = vi.fn();
    render(<ProjectList projects={projects} onSelect={() => {}} onToggleFavorite={onToggleFavorite} />);

    const favoritesSection = screen.getByRole("region", { name: /favorites/i });
    const lastBeaconRow = within(favoritesSection).getByText("last-beacon").closest("li")!;
    fireEvent.click(within(lastBeaconRow).getByRole("button", { name: /toggle favorite/i }));

    expect(onToggleFavorite).toHaveBeenCalledWith("pixel-perfect/last-beacon", false);
  });

  it("calls onSelect when a project is clicked in the all-projects section", () => {
    const onSelect = vi.fn();
    render(<ProjectList projects={projects} onSelect={onSelect} onToggleFavorite={() => {}} />);

    const allSection = screen.getByRole("region", { name: /all projects/i });
    fireEvent.click(within(allSection).getByText("last-beacon"));

    expect(onSelect).toHaveBeenCalledWith("pixel-perfect/last-beacon");
  });
});
