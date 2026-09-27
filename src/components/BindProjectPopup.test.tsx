import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { BindProjectPopup } from "./BindProjectPopup";
import type { Project } from "../api/projects";

const projects: Project[] = [
  { full_name: "org/last-beacon", owner: "org", name: "last-beacon", favorite: true },
  { full_name: "org/other-game", owner: "org", name: "other-game", favorite: false },
];

const noopBindUrl = async () => {};

describe("BindProjectPopup", () => {
  it("lists every candidate project", () => {
    render(
      <BindProjectPopup projects={projects} loggedIn onBindUrl={noopBindUrl} onBind={() => {}} onToggleFavorite={() => {}} onClose={() => {}} />,
    );

    expect(screen.getByRole("button", { name: "last-beacon" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "other-game" })).toBeInTheDocument();
  });

  it("filters the list by search text", () => {
    render(
      <BindProjectPopup projects={projects} loggedIn onBindUrl={noopBindUrl} onBind={() => {}} onToggleFavorite={() => {}} onClose={() => {}} />,
    );

    fireEvent.change(screen.getByLabelText("Search projects"), { target: { value: "other" } });

    expect(screen.queryByRole("button", { name: "last-beacon" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "other-game" })).toBeInTheDocument();
  });

  it("calls onBind with the project's full name when clicked", () => {
    const onBind = vi.fn();
    render(
      <BindProjectPopup projects={projects} loggedIn onBindUrl={noopBindUrl} onBind={onBind} onToggleFavorite={() => {}} onClose={() => {}} />,
    );

    fireEvent.click(screen.getByRole("button", { name: "last-beacon" }));

    expect(onBind).toHaveBeenCalledWith("org/last-beacon");
  });

  it("calls onToggleFavorite when a star is clicked", () => {
    const onToggleFavorite = vi.fn();
    render(
      <BindProjectPopup
        projects={projects}
        loggedIn
        onBindUrl={noopBindUrl}
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
      <BindProjectPopup projects={projects} loggedIn onBindUrl={noopBindUrl} onBind={() => {}} onToggleFavorite={() => {}} onClose={onClose} />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Close" }));

    expect(onClose).toHaveBeenCalled();
  });

  it("submits a pasted repo URL via onBindUrl", async () => {
    const onBindUrl = vi.fn().mockResolvedValue(undefined);
    render(
      <BindProjectPopup
        projects={projects}
        loggedIn
        onBind={() => {}}
        onBindUrl={onBindUrl}
        onToggleFavorite={() => {}}
        onClose={() => {}}
      />,
    );

    fireEvent.change(screen.getByLabelText("Or bind any public repo by URL"), {
      target: { value: "  https://github.com/someone/public-game  " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Bind" }));

    await waitFor(() => expect(onBindUrl).toHaveBeenCalledWith("https://github.com/someone/public-game"));
  });

  it("disables the URL Bind button until something is entered", () => {
    render(
      <BindProjectPopup
        projects={projects}
        loggedIn
        onBind={() => {}}
        onBindUrl={noopBindUrl}
        onToggleFavorite={() => {}}
        onClose={() => {}}
      />,
    );

    expect(screen.getByRole("button", { name: "Bind" })).toBeDisabled();
  });

  it("shows the error when binding by URL fails", async () => {
    const onBindUrl = vi.fn().mockRejectedValue("Repository someone/nothing was not found.");
    render(
      <BindProjectPopup
        projects={projects}
        loggedIn
        onBind={() => {}}
        onBindUrl={onBindUrl}
        onToggleFavorite={() => {}}
        onClose={() => {}}
      />,
    );

    fireEvent.change(screen.getByLabelText("Or bind any public repo by URL"), {
      target: { value: "https://github.com/someone/nothing" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Bind" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Repository someone/nothing was not found.");
  });

  it("logged out, hides the project list and offers only URL binding", () => {
    render(
      <BindProjectPopup
        projects={projects}
        loggedIn={false}
        onBind={() => {}}
        onBindUrl={noopBindUrl}
        onToggleFavorite={() => {}}
        onClose={() => {}}
      />,
    );

    expect(screen.queryByLabelText("Search projects")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "last-beacon" })).not.toBeInTheDocument();
    expect(screen.getByText(/only public repositories can be bound/)).toBeInTheDocument();
    expect(screen.getByLabelText("Bind a public repo by URL")).toBeInTheDocument();
  });
});
