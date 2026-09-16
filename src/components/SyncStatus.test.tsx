import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { SyncStatus } from "./SyncStatus";

describe("SyncStatus", () => {
  it("renders nothing when idle", () => {
    const { container } = render(<SyncStatus state={{ phase: "idle" }} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows download percentage while syncing", () => {
    render(<SyncStatus state={{ phase: "syncing", downloaded: 500, total: 1000 }} />);
    expect(screen.getByText(/50%/)).toBeInTheDocument();
  });

  it("shows a finishing-up message once fully downloaded but not yet done", () => {
    render(<SyncStatus state={{ phase: "syncing", downloaded: 1000, total: 1000 }} />);
    expect(screen.getByText(/finishing up/i)).toBeInTheDocument();
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
