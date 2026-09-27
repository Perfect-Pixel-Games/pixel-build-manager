import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { BusyOverlay } from "./BusyOverlay";

describe("BusyOverlay", () => {
  it("always renders its children", () => {
    render(
      <BusyOverlay active={false}>
        <button>Sync</button>
      </BusyOverlay>,
    );

    expect(screen.getByRole("button", { name: "Sync" })).toBeInTheDocument();
  });

  it("renders no status/throbber when inactive", () => {
    render(
      <BusyOverlay active={false}>
        <button>Sync</button>
      </BusyOverlay>,
    );

    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("renders a status region with the label when active, alongside the still-mounted children", () => {
    render(
      <BusyOverlay active label="Downloading shipping.zip: 50%">
        <button>Sync</button>
      </BusyOverlay>,
    );

    expect(screen.getByRole("status")).toHaveTextContent("Downloading shipping.zip: 50%");
    expect(screen.getByRole("button", { name: "Sync" })).toBeInTheDocument();
  });

  it("marks the content inert when active, so it can't be interacted with underneath the overlay", () => {
    const { container } = render(
      <BusyOverlay active label="Working...">
        <button>Sync</button>
      </BusyOverlay>,
    );

    expect(container.querySelector(".busy-overlay-content")).toHaveAttribute("inert");
  });
});
