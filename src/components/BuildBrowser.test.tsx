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

function renderBrowser(overrides: Partial<React.ComponentProps<typeof BuildBrowser>> = {}) {
  return render(
    <BuildBrowser
      releases={releases}
      syncMode="manual"
      selectedReleaseTag={null}
      onSelectRelease={noop}
      onSelectLatest={noop}
      tickedConfigs={[]}
      onToggleConfig={noop}
      syncedConfigs={[]}
      onSync={noop}
      onSyncLatest={noop}
      disabled={false}
      {...overrides}
    />,
  );
}

describe("BuildBrowser", () => {
  it("lists releases and prereleases, labeling prereleases distinctly", () => {
    renderBrowser();

    expect(screen.getByRole("button", { name: "LastBeacon 0.2.14" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "LastBeacon 0.2.13" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "LastBeacon 0.3.0-rc1 (prerelease)" })).toBeInTheDocument();
  });

  it("always shows the pinned Latest release and Latest prerelease rows", () => {
    renderBrowser();

    expect(screen.getByRole("button", { name: "Latest release" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Latest prerelease" })).toBeInTheDocument();
  });

  it("filters the release list by the search box", () => {
    renderBrowser();

    fireEvent.change(screen.getByLabelText("Search releases"), { target: { value: "0.2.13" } });

    expect(screen.queryByRole("button", { name: "LastBeacon 0.2.14" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "LastBeacon 0.2.13" })).toBeInTheDocument();
  });

  it("lists every build config across all releases, including prerelease-only ones", () => {
    renderBrowser();

    expect(screen.getByLabelText("shipping.zip")).toBeInTheDocument();
    expect(screen.getByLabelText("test.zip")).toBeInTheDocument();
    expect(screen.getByLabelText("ps4.zip")).toBeInTheDocument();
  });

  it("calls onSelectRelease when a release row is clicked", () => {
    const onSelectRelease = vi.fn();
    renderBrowser({ onSelectRelease });

    fireEvent.click(screen.getByRole("button", { name: "LastBeacon 0.2.14" }));

    expect(onSelectRelease).toHaveBeenCalledWith("0.2.14");
  });

  it("marks the selected release's button as pressed", () => {
    renderBrowser({ selectedReleaseTag: "0.2.13" });

    expect(screen.getByRole("button", { name: "LastBeacon 0.2.13" })).toHaveAttribute("aria-pressed", "true");
  });

  it("disables a config checkbox when the selected release has no matching asset", () => {
    renderBrowser({ selectedReleaseTag: "0.2.13" });

    // 0.2.13 only has shipping.zip -- test.zip must be disabled for it.
    expect(screen.getByLabelText("shipping.zip")).not.toBeDisabled();
    expect(screen.getByLabelText("test.zip")).toBeDisabled();
  });

  it("leaves every config checkbox enabled when no release is selected yet", () => {
    renderBrowser();

    expect(screen.getByLabelText("shipping.zip")).not.toBeDisabled();
    expect(screen.getByLabelText("test.zip")).not.toBeDisabled();
  });

  it("calls onToggleConfig with the config name and new checked state", () => {
    const onToggleConfig = vi.fn();
    renderBrowser({ onToggleConfig });

    fireEvent.click(screen.getByLabelText("shipping.zip"));

    expect(onToggleConfig).toHaveBeenCalledWith("shipping.zip", true);
  });

  it("disables the sync button when nothing ticked is available for the selected release", () => {
    renderBrowser({ selectedReleaseTag: "0.2.14" });

    expect(screen.getByRole("button", { name: "Sync" })).toBeDisabled();
  });

  it("shows Sync and calls onSync with only the missing ticked assets", () => {
    const onSync = vi.fn();
    renderBrowser({
      selectedReleaseTag: "0.2.14",
      tickedConfigs: ["shipping.zip", "test.zip"],
      syncedConfigs: ["test.zip"],
      onSync,
    });

    fireEvent.click(screen.getByRole("button", { name: "Sync" }));

    expect(onSync).toHaveBeenCalledWith(releases[0], [releases[0].assets[0]]);
  });

  it("shows a checkmark and re-verifies every ticked asset once fully synced", () => {
    const onSync = vi.fn();
    renderBrowser({
      selectedReleaseTag: "0.2.14",
      tickedConfigs: ["shipping.zip"],
      syncedConfigs: ["shipping.zip"],
      onSync,
    });

    fireEvent.click(screen.getByRole("button", { name: "✓ Synced" }));

    expect(onSync).toHaveBeenCalledWith(releases[0], [releases[0].assets[0]]);
  });

  it("disables the search box, release rows, checkboxes, and sync button when disabled", () => {
    renderBrowser({ selectedReleaseTag: "0.2.14", tickedConfigs: ["shipping.zip"], disabled: true });

    expect(screen.getByLabelText("Search releases")).toBeDisabled();
    expect(screen.getByRole("button", { name: "LastBeacon 0.2.14" })).toBeDisabled();
    expect(screen.getByLabelText("shipping.zip")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Sync" })).toBeDisabled();
  });

  describe("latest mode", () => {
    it("marks Latest release pressed and no concrete release pressed when syncMode is latest_release", () => {
      renderBrowser({ syncMode: "latest_release", selectedReleaseTag: "0.2.13" });

      expect(screen.getByRole("button", { name: "Latest release" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "LastBeacon 0.2.13" })).toHaveAttribute("aria-pressed", "false");
    });

    it("clicking the Latest prerelease row calls onSelectLatest", () => {
      const onSelectLatest = vi.fn();
      renderBrowser({ onSelectLatest });

      fireEvent.click(screen.getByRole("button", { name: "Latest prerelease" }));

      expect(onSelectLatest).toHaveBeenCalledWith("latest_prerelease");
    });

    it("resolves available configs against the most recently published matching release", () => {
      // latest_release should resolve to 0.2.14 (most recent non-prerelease),
      // not 0.3.0-rc1 (most recent overall, but a prerelease).
      renderBrowser({ syncMode: "latest_release", tickedConfigs: ["shipping.zip", "test.zip"] });

      expect(screen.getByLabelText("shipping.zip")).not.toBeDisabled();
      expect(screen.getByLabelText("test.zip")).not.toBeDisabled();
    });

    it("resolves Latest prerelease's available configs to the prerelease release's assets", () => {
      renderBrowser({ syncMode: "latest_prerelease", tickedConfigs: ["shipping.zip", "ps4.zip"] });

      expect(screen.getByLabelText("ps4.zip")).not.toBeDisabled();
      expect(screen.getByLabelText("shipping.zip")).toBeDisabled();
    });

    it("disables Sync when nothing is ticked in latest mode", () => {
      renderBrowser({ syncMode: "latest_release" });

      expect(screen.getByRole("button", { name: "Sync" })).toBeDisabled();
    });

    it("clicking Sync in latest mode calls onSyncLatest instead of onSync", () => {
      const onSync = vi.fn();
      const onSyncLatest = vi.fn();
      renderBrowser({ syncMode: "latest_release", tickedConfigs: ["shipping.zip"], onSync, onSyncLatest });

      fireEvent.click(screen.getByRole("button", { name: "Sync" }));

      expect(onSyncLatest).toHaveBeenCalled();
      expect(onSync).not.toHaveBeenCalled();
    });

    it("shows the checkmark in latest mode once every ticked config is synced", () => {
      renderBrowser({ syncMode: "latest_release", tickedConfigs: ["shipping.zip"], syncedConfigs: ["shipping.zip"] });

      expect(screen.getByRole("button", { name: "✓ Synced" })).toBeInTheDocument();
    });
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
      renderBrowser({ releases: versionedReleases });

      expect(screen.getAllByLabelText("pixel-build-manager_x64-setup.exe")).toHaveLength(1);
    });

    it("ticking the collapsed config and syncing passes the selected release's actual asset", () => {
      const onSync = vi.fn();
      renderBrowser({
        releases: versionedReleases,
        selectedReleaseTag: "0.0.3",
        tickedConfigs: ["pixel-build-manager_x64-setup.exe"],
        onSync,
      });

      fireEvent.click(screen.getByRole("button", { name: "Sync" }));

      expect(onSync).toHaveBeenCalledWith(versionedReleases[0], [versionedReleases[0].assets[0]]);
    });
  });
});
