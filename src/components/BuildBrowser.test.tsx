import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { BuildBrowser } from "./BuildBrowser";
import type { Release } from "../api/projects";

const releases: Release[] = [
  {
    id: 1,
    tag_name: "0.2.14",
    name: "LastBeacon 0.2.14",
    prerelease: false,
    published_at: "2026-09-01T00:00:00Z",
    assets: [
      { id: 10, name: "shipping.zip", size: 100, browser_download_url: "https://example.com/a" },
      { id: 11, name: "test.zip", size: 100, browser_download_url: "https://example.com/b" },
    ],
  },
  {
    id: 2,
    tag_name: "0.2.13",
    name: "LastBeacon 0.2.13",
    prerelease: false,
    published_at: "2026-08-01T00:00:00Z",
    assets: [{ id: 8, name: "shipping.zip", size: 100, browser_download_url: "https://example.com/c" }],
  },
  {
    id: 3,
    tag_name: "0.3.0-rc1",
    name: "LastBeacon 0.3.0-rc1",
    prerelease: true,
    published_at: "2026-09-10T00:00:00Z",
    assets: [{ id: 30, name: "ps4.zip", size: 100, browser_download_url: "https://example.com/rc" }],
  },
];

const noop = () => {};

describe("BuildBrowser", () => {
  it("lists releases and prereleases, labeling prereleases distinctly", () => {
    render(
      <BuildBrowser
        releases={releases}
        selectedReleaseTag={null}
        onSelectRelease={noop}
        tickedConfigs={[]}
        onToggleConfig={noop}
        syncedConfigs={[]}
        onSync={noop}
        disabled={false}
      />,
    );

    expect(screen.getByRole("button", { name: "LastBeacon 0.2.14" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "LastBeacon 0.2.13" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "LastBeacon 0.3.0-rc1 (prerelease)" })).toBeInTheDocument();
  });

  it("filters the release list by the search box", () => {
    render(
      <BuildBrowser
        releases={releases}
        selectedReleaseTag={null}
        onSelectRelease={noop}
        tickedConfigs={[]}
        onToggleConfig={noop}
        syncedConfigs={[]}
        onSync={noop}
        disabled={false}
      />,
    );

    fireEvent.change(screen.getByLabelText("Search releases"), { target: { value: "0.2.13" } });

    expect(screen.queryByRole("button", { name: "LastBeacon 0.2.14" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "LastBeacon 0.2.13" })).toBeInTheDocument();
  });

  it("lists every build config across all releases, including prerelease-only ones", () => {
    render(
      <BuildBrowser
        releases={releases}
        selectedReleaseTag={null}
        onSelectRelease={noop}
        tickedConfigs={[]}
        onToggleConfig={noop}
        syncedConfigs={[]}
        onSync={noop}
        disabled={false}
      />,
    );

    expect(screen.getByLabelText("shipping.zip")).toBeInTheDocument();
    expect(screen.getByLabelText("test.zip")).toBeInTheDocument();
    expect(screen.getByLabelText("ps4.zip")).toBeInTheDocument();
  });

  it("calls onSelectRelease when a release row is clicked", () => {
    const onSelectRelease = vi.fn();
    render(
      <BuildBrowser
        releases={releases}
        selectedReleaseTag={null}
        onSelectRelease={onSelectRelease}
        tickedConfigs={[]}
        onToggleConfig={noop}
        syncedConfigs={[]}
        onSync={noop}
        disabled={false}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "LastBeacon 0.2.14" }));

    expect(onSelectRelease).toHaveBeenCalledWith("0.2.14");
  });

  it("marks the selected release's button as pressed", () => {
    render(
      <BuildBrowser
        releases={releases}
        selectedReleaseTag="0.2.13"
        onSelectRelease={noop}
        tickedConfigs={[]}
        onToggleConfig={noop}
        syncedConfigs={[]}
        onSync={noop}
        disabled={false}
      />,
    );

    expect(screen.getByRole("button", { name: "LastBeacon 0.2.13" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("disables a config checkbox when the selected release has no matching asset", () => {
    render(
      <BuildBrowser
        releases={releases}
        selectedReleaseTag="0.2.13"
        onSelectRelease={noop}
        tickedConfigs={[]}
        onToggleConfig={noop}
        syncedConfigs={[]}
        onSync={noop}
        disabled={false}
      />,
    );

    // 0.2.13 only has shipping.zip -- test.zip must be disabled for it.
    expect(screen.getByLabelText("shipping.zip")).not.toBeDisabled();
    expect(screen.getByLabelText("test.zip")).toBeDisabled();
  });

  it("leaves every config checkbox enabled when no release is selected yet", () => {
    render(
      <BuildBrowser
        releases={releases}
        selectedReleaseTag={null}
        onSelectRelease={noop}
        tickedConfigs={[]}
        onToggleConfig={noop}
        syncedConfigs={[]}
        onSync={noop}
        disabled={false}
      />,
    );

    expect(screen.getByLabelText("shipping.zip")).not.toBeDisabled();
    expect(screen.getByLabelText("test.zip")).not.toBeDisabled();
  });

  it("calls onToggleConfig with the config name and new checked state", () => {
    const onToggleConfig = vi.fn();
    render(
      <BuildBrowser
        releases={releases}
        selectedReleaseTag={null}
        onSelectRelease={noop}
        tickedConfigs={[]}
        onToggleConfig={onToggleConfig}
        syncedConfigs={[]}
        onSync={noop}
        disabled={false}
      />,
    );

    fireEvent.click(screen.getByLabelText("shipping.zip"));

    expect(onToggleConfig).toHaveBeenCalledWith("shipping.zip", true);
  });

  it("disables the sync button when nothing ticked is available for the selected release", () => {
    render(
      <BuildBrowser
        releases={releases}
        selectedReleaseTag="0.2.14"
        onSelectRelease={noop}
        tickedConfigs={[]}
        onToggleConfig={noop}
        syncedConfigs={[]}
        onSync={noop}
        disabled={false}
      />,
    );

    expect(screen.getByRole("button", { name: "Sync" })).toBeDisabled();
  });

  it("shows Sync and calls onSync with only the missing ticked assets", () => {
    const onSync = vi.fn();
    render(
      <BuildBrowser
        releases={releases}
        selectedReleaseTag="0.2.14"
        onSelectRelease={noop}
        tickedConfigs={["shipping.zip", "test.zip"]}
        onToggleConfig={noop}
        syncedConfigs={["test.zip"]}
        onSync={onSync}
        disabled={false}
      />,
    );

    const button = screen.getByRole("button", { name: "Sync" });
    fireEvent.click(button);

    expect(onSync).toHaveBeenCalledWith(releases[0], [releases[0].assets[0]]);
  });

  it("shows a checkmark and re-verifies every ticked asset once fully synced", () => {
    const onSync = vi.fn();
    render(
      <BuildBrowser
        releases={releases}
        selectedReleaseTag="0.2.14"
        onSelectRelease={noop}
        tickedConfigs={["shipping.zip"]}
        onToggleConfig={noop}
        syncedConfigs={["shipping.zip"]}
        onSync={onSync}
        disabled={false}
      />,
    );

    const button = screen.getByRole("button", { name: "✓ Synced" });
    fireEvent.click(button);

    expect(onSync).toHaveBeenCalledWith(releases[0], [releases[0].assets[0]]);
  });

  it("disables the search box, release rows, checkboxes, and sync button when disabled", () => {
    render(
      <BuildBrowser
        releases={releases}
        selectedReleaseTag="0.2.14"
        onSelectRelease={noop}
        tickedConfigs={["shipping.zip"]}
        onToggleConfig={noop}
        syncedConfigs={[]}
        onSync={noop}
        disabled
      />,
    );

    expect(screen.getByLabelText("Search releases")).toBeDisabled();
    expect(screen.getByRole("button", { name: "LastBeacon 0.2.14" })).toBeDisabled();
    expect(screen.getByLabelText("shipping.zip")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Sync" })).toBeDisabled();
  });

  describe("projects whose asset names embed the release version", () => {
    // Mirrors this app's own Tauri-built installer naming convention
    // ("pixel-build-manager_0.0.3_x64-setup.exe") -- without stripping the
    // release's own version out of the asset name first, every release
    // would register as a brand new build config instead of the one config
    // it actually is.
    const versionedReleases: Release[] = [
      {
        id: 1,
        tag_name: "0.0.3",
        name: "Pixel Build Manager 0.0.3",
        prerelease: true,
        published_at: "2026-09-23T00:00:00Z",
        assets: [
          {
            id: 100,
            name: "pixel-build-manager_0.0.3_x64-setup.exe",
            size: 100,
            browser_download_url: "https://example.com/a",
          },
        ],
      },
      {
        id: 2,
        tag_name: "0.0.2",
        name: "Pixel Build Manager 0.0.2",
        prerelease: true,
        published_at: "2026-09-20T00:00:00Z",
        assets: [
          {
            id: 90,
            name: "pixel-build-manager_0.0.2_x64-setup.exe",
            size: 100,
            browser_download_url: "https://example.com/b",
          },
        ],
      },
    ];

    it("collapses per-release versioned asset names into a single build config", () => {
      render(
        <BuildBrowser
          releases={versionedReleases}
          selectedReleaseTag={null}
          onSelectRelease={noop}
          tickedConfigs={[]}
          onToggleConfig={noop}
          syncedConfigs={[]}
          onSync={noop}
          disabled={false}
        />,
      );

      expect(screen.getAllByLabelText("pixel-build-manager_x64-setup.exe")).toHaveLength(1);
    });

    it("ticking the collapsed config and syncing passes the selected release's actual asset", () => {
      const onSync = vi.fn();
      render(
        <BuildBrowser
          releases={versionedReleases}
          selectedReleaseTag="0.0.3"
          onSelectRelease={noop}
          tickedConfigs={["pixel-build-manager_x64-setup.exe"]}
          onToggleConfig={noop}
          syncedConfigs={[]}
          onSync={onSync}
          disabled={false}
        />,
      );

      fireEvent.click(screen.getByRole("button", { name: "Sync" }));

      expect(onSync).toHaveBeenCalledWith(versionedReleases[0], [versionedReleases[0].assets[0]]);
    });
  });
});
