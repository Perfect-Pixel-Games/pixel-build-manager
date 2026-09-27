import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ThemeToggle } from "./ThemeToggle";

describe("ThemeToggle", () => {
  it("renders a button for each theme option", () => {
    render(<ThemeToggle theme="system" onChange={() => {}} />);

    expect(screen.getByRole("button", { name: "Light" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "System" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Dark" })).toBeInTheDocument();
  });

  it("marks the current theme's button as pressed", () => {
    render(<ThemeToggle theme="dark" onChange={() => {}} />);

    expect(screen.getByRole("button", { name: "Dark" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "false");
  });

  it("calls onChange with the clicked theme", () => {
    const onChange = vi.fn();
    render(<ThemeToggle theme="system" onChange={onChange} />);

    fireEvent.click(screen.getByRole("button", { name: "Dark" }));

    expect(onChange).toHaveBeenCalledWith("dark");
  });
});
