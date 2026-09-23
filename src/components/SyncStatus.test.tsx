import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { SyncStatus } from "./SyncStatus";

describe("SyncStatus", () => {
  it("renders nothing when idle", () => {
    const { container } = render(<SyncStatus state={{ phase: "idle" }} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing while syncing (the busy overlay owns that display)", () => {
    const { container } = render(
      <SyncStatus state={{ phase: "syncing", configName: "shipping.zip", downloaded: 500, total: 1000 }} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("shows a success message when done", () => {
    render(<SyncStatus state={{ phase: "done" }} />);
    expect(screen.getByText(/synced/i)).toBeInTheDocument();
  });

  it("shows the error message on failure", () => {
    render(<SyncStatus state={{ phase: "error", message: "network down" }} />);
    expect(screen.getByText(/network down/)).toBeInTheDocument();
  });
});
