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

function openDropdown() {
  fireEvent.click(screen.getByRole("button", { expanded: false }));
}

describe("ReleaseList", () => {
  it("shows a placeholder on the dropdown toggle when nothing is active", () => {
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

    expect(screen.getByRole("button", { expanded: false })).toHaveTextContent("No active build");
  });

  it("shows the active build as the dropdown's collapsed value", () => {
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

    const toggle = screen.getByRole("button", { expanded: false });
    expect(toggle).toHaveTextContent("LastBeacon 0.2.14");
    expect(toggle).toHaveTextContent("last-beacon-windows-x64-shipping.tar.gz");
  });

  it("lists every asset of every release as an option once opened", () => {
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

    expect(screen.queryByText(/last-beacon-windows-x64-shipping\.tar\.gz/)).not.toBeInTheDocument();

    openDropdown();

    expect(screen.getByText(/last-beacon-windows-x64-shipping\.tar\.gz/)).toBeInTheDocument();
    expect(screen.getByText(/last-beacon-windows-x64-test\.tar\.gz/)).toBeInTheDocument();
  });

  it("only marks the active asset's own option, not every option sharing that asset name", () => {
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

    openDropdown();

    const rows = screen.getAllByText(/last-beacon-windows-x64-shipping\.tar\.gz/).map((el) => el.closest("li"));
    const activeRows = rows.filter((row) => row?.textContent?.includes("Active"));
    expect(activeRows).toHaveLength(1);
    expect(activeRows[0]).toHaveTextContent("LastBeacon 0.2.14");
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

    openDropdown();

    expect(
      screen.getByRole("button", { name: "Delete downloaded last-beacon-windows-x64-shipping.tar.gz" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Delete downloaded last-beacon-windows-x64-test.tar.gz" }),
    ).not.toBeInTheDocument();
  });

  it("shows Check instead of Sync for an asset that's already downloaded", () => {
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

    openDropdown();

    const syncButton = screen.getByRole("button", { name: "Sync last-beacon-windows-x64-shipping.tar.gz" });
    expect(syncButton).toHaveTextContent("Check");

    const notYetDownloadedButton = screen.getByRole("button", {
      name: "Sync last-beacon-windows-x64-test.tar.gz",
    });
    expect(notYetDownloadedButton).toHaveTextContent("Sync");
  });

  it("still calls onSync (to re-check/re-download) when the Check button is clicked", () => {
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

    openDropdown();
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

    openDropdown();
    fireEvent.click(
      screen.getByRole("button", { name: "Delete downloaded last-beacon-windows-x64-shipping.tar.gz" }),
    );

    expect(onDelete).toHaveBeenCalledWith(releases[0], 10);
  });

  it("toggles the option list closed when the dropdown button is clicked again", () => {
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

    openDropdown();
    expect(screen.getByText(/last-beacon-windows-x64-shipping\.tar\.gz/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { expanded: true }));

    expect(screen.queryByText(/last-beacon-windows-x64-shipping\.tar\.gz/)).not.toBeInTheDocument();
  });
});
