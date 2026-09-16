import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { ReleaseList } from "./ReleaseList";
import type { Release } from "../api/projects";

const releases: Release[] = [
  {
    id: 1,
    tag_name: "0.2.14",
    name: "LastBeacon 0.2.14",
    prerelease: false,
    published_at: "2026-09-01T00:00:00Z",
    assets: [
      { id: 10, name: "last-beacon-windows-x64-shipping.tar.gz", size: 12345, browser_download_url: "https://example.com/a" },
      { id: 11, name: "last-beacon-windows-x64-test.tar.gz", size: 12345, browser_download_url: "https://example.com/b" },
    ],
  },
];

describe("ReleaseList", () => {
  it("lists every asset of every release as its own syncable row", () => {
    render(<ReleaseList releases={releases} activeAssetName={null} onSync={() => {}} />);

    expect(screen.getByText("LastBeacon 0.2.14")).toBeInTheDocument();
    expect(screen.getByText("last-beacon-windows-x64-shipping.tar.gz")).toBeInTheDocument();
    expect(screen.getByText("last-beacon-windows-x64-test.tar.gz")).toBeInTheDocument();
  });

  it("marks the active asset", () => {
    render(
      <ReleaseList
        releases={releases}
        activeAssetName="last-beacon-windows-x64-shipping.tar.gz"
        onSync={() => {}}
      />,
    );

    const activeRow = screen.getByText("last-beacon-windows-x64-shipping.tar.gz").closest("li");
    expect(activeRow).toHaveTextContent("Active");
  });
});
