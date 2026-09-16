import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
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
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag={null}
        activeAssetName={null}
        cachedAssetIds={new Set()}
        onSync={() => {}}
        onDelete={() => {}}
      />,
    );

    expect(screen.getByText("LastBeacon 0.2.14")).toBeInTheDocument();
    expect(screen.getByText("last-beacon-windows-x64-shipping.tar.gz")).toBeInTheDocument();
    expect(screen.getByText("last-beacon-windows-x64-test.tar.gz")).toBeInTheDocument();
  });

  it("marks the active asset", () => {
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag="0.2.14"
        activeAssetName="last-beacon-windows-x64-shipping.tar.gz"
        cachedAssetIds={new Set()}
        onSync={() => {}}
        onDelete={() => {}}
      />,
    );

    const activeRow = screen.getByText("last-beacon-windows-x64-shipping.tar.gz").closest("li");
    expect(activeRow).toHaveTextContent("Active");
  });

  it("only marks the active asset on its own release, not every release sharing that asset name", () => {
    const twoReleases: Release[] = [
      {
        id: 1,
        tag_name: "0.2.14",
        name: "LastBeacon 0.2.14",
        prerelease: false,
        published_at: "2026-09-01T00:00:00Z",
        assets: [
          { id: 10, name: "last-beacon-windows-x64-shipping.tar.gz", size: 12345, browser_download_url: "https://example.com/a" },
        ],
      },
      {
        id: 2,
        tag_name: "0.2.13",
        name: "LastBeacon 0.2.13",
        prerelease: false,
        published_at: "2026-08-01T00:00:00Z",
        assets: [
          { id: 9, name: "last-beacon-windows-x64-shipping.tar.gz", size: 12345, browser_download_url: "https://example.com/c" },
        ],
      },
    ];

    render(
      <ReleaseList
        releases={twoReleases}
        activeReleaseTag="0.2.14"
        activeAssetName="last-beacon-windows-x64-shipping.tar.gz"
        cachedAssetIds={new Set()}
        onSync={() => {}}
        onDelete={() => {}}
      />,
    );

    const rows = screen.getAllByText("last-beacon-windows-x64-shipping.tar.gz").map((el) => el.closest("li"));
    const activeRows = rows.filter((row) => row?.textContent?.includes("Active"));
    expect(activeRows).toHaveLength(1);

    const activeReleaseLi = activeRows[0]?.parentElement?.closest("li");
    expect(activeReleaseLi).toHaveTextContent("LastBeacon 0.2.14");
  });

  it("only shows a delete button for assets that are actually cached", () => {
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag={null}
        activeAssetName={null}
        cachedAssetIds={new Set([10])}
        onSync={() => {}}
        onDelete={() => {}}
      />,
    );

    expect(
      screen.getByRole("button", { name: "Delete downloaded last-beacon-windows-x64-shipping.tar.gz" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Delete downloaded last-beacon-windows-x64-test.tar.gz" }),
    ).not.toBeInTheDocument();
  });

  it("shows a checkmark instead of Sync for an asset that's already downloaded", () => {
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag={null}
        activeAssetName={null}
        cachedAssetIds={new Set([10])}
        onSync={() => {}}
        onDelete={() => {}}
      />,
    );

    const syncButton = screen.getByRole("button", { name: "Sync last-beacon-windows-x64-shipping.tar.gz" });
    expect(syncButton).toHaveTextContent("✓");

    const notYetDownloadedButton = screen.getByRole("button", {
      name: "Sync last-beacon-windows-x64-test.tar.gz",
    });
    expect(notYetDownloadedButton).toHaveTextContent("Sync");
  });

  it("still calls onSync (to re-check/re-download) when the checkmark button is clicked", () => {
    const onSync = vi.fn();
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag={null}
        activeAssetName={null}
        cachedAssetIds={new Set([10])}
        onSync={onSync}
        onDelete={() => {}}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Sync last-beacon-windows-x64-shipping.tar.gz" }));

    expect(onSync).toHaveBeenCalledWith(releases[0], 10);
  });

  it("calls onDelete with the release and asset id when delete is clicked", () => {
    const onDelete = vi.fn();
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag={null}
        activeAssetName={null}
        cachedAssetIds={new Set([10])}
        onSync={() => {}}
        onDelete={onDelete}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Delete downloaded last-beacon-windows-x64-shipping.tar.gz" }),
    );

    expect(onDelete).toHaveBeenCalledWith(releases[0], 10);
  });
});
