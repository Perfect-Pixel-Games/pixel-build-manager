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

function selectBuildType(name: string) {
  fireEvent.change(screen.getByLabelText("Build type"), { target: { value: name } });
}

const noop = () => {};

describe("ReleaseList", () => {
  describe("build type selector", () => {
    it("lists the unique asset names across all releases as build type options", () => {
      render(
        <ReleaseList
          releases={releases}
          activeReleaseTag={null}
          activeAssetName={null}
          cachedAssetIds={new Set()}
          onSelect={noop}
          onCheck={noop}
          onDelete={noop}
        />,
      );

      expect(screen.getByRole("option", { name: "last-beacon-windows-x64-shipping" })).toBeInTheDocument();
      expect(screen.getByRole("option", { name: "last-beacon-windows-x64-test" })).toBeInTheDocument();
    });

    it("strips the known archive extension from the build type option labels", () => {
      render(
        <ReleaseList
          releases={releases}
          activeReleaseTag={null}
          activeAssetName={null}
          cachedAssetIds={new Set()}
          onSelect={noop}
          onCheck={noop}
          onDelete={noop}
        />,
      );

      expect(screen.queryByRole("option", { name: /\.tar\.gz/ })).not.toBeInTheDocument();
    });

    it("defaults to the active build's type when there is an active build", () => {
      render(
        <ReleaseList
          releases={releases}
          activeReleaseTag="0.2.14"
          activeAssetName="last-beacon-windows-x64-shipping.tar.gz"
          cachedAssetIds={new Set()}
          onSelect={noop}
          onCheck={noop}
          onDelete={noop}
        />,
      );

      expect(screen.getByLabelText("Build type")).toHaveValue("last-beacon-windows-x64-shipping.tar.gz");
    });

    it("defaults to the only build type when there's no ambiguity", () => {
      const singleTypeReleases: Release[] = [{ ...releases[0], assets: [releases[0].assets[0]] }];

      render(
        <ReleaseList
          releases={singleTypeReleases}
          activeReleaseTag={null}
          activeAssetName={null}
          cachedAssetIds={new Set()}
          onSelect={noop}
          onCheck={noop}
          onDelete={noop}
        />,
      );

      expect(screen.getByLabelText("Build type")).toHaveValue("last-beacon-windows-x64-shipping.tar.gz");
    });

    it("requires the user to pick a type when there's no active build and multiple types exist", () => {
      render(
        <ReleaseList
          releases={releases}
          activeReleaseTag={null}
          activeAssetName={null}
          cachedAssetIds={new Set()}
          onSelect={noop}
          onCheck={noop}
          onDelete={noop}
        />,
      );

      expect(screen.getByLabelText("Build type")).toHaveValue("");
      expect(screen.getByText("Select a build type to see available versions.")).toBeInTheDocument();
      expect(screen.queryByRole("button", { expanded: false })).not.toBeInTheDocument();
    });

    it("filters the option list down to the selected build type, across releases", () => {
      const twoReleases: Release[] = [
        releases[0],
        {
          id: 2,
          tag_name: "0.2.13",
          name: "LastBeacon 0.2.13",
          prerelease: false,
          published_at: "2026-08-01T00:00:00Z",
          assets: [
            { id: 8, name: "last-beacon-windows-x64-shipping.tar.gz", size: 12345, browser_download_url: "https://example.com/c" },
          ],
        },
      ];

      render(
        <ReleaseList
          releases={twoReleases}
          activeReleaseTag={null}
          activeAssetName={null}
          cachedAssetIds={new Set()}
          onSelect={noop}
          onCheck={noop}
          onDelete={noop}
        />,
      );

      selectBuildType("last-beacon-windows-x64-shipping.tar.gz");
      openDropdown();

      expect(screen.getByRole("button", { name: "LastBeacon 0.2.14" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "LastBeacon 0.2.13" })).toBeInTheDocument();
    });

    it("switching build types updates the option list", () => {
      render(
        <ReleaseList
          releases={releases}
          activeReleaseTag={null}
          activeAssetName={null}
          cachedAssetIds={new Set()}
          onSelect={noop}
          onCheck={noop}
          onDelete={noop}
        />,
      );

      selectBuildType("last-beacon-windows-x64-shipping.tar.gz");
      openDropdown();
      expect(
        screen.getByRole("button", { name: "Sync last-beacon-windows-x64-shipping.tar.gz" }),
      ).toBeInTheDocument();
      expect(
        screen.queryByRole("button", { name: "Sync last-beacon-windows-x64-test.tar.gz" }),
      ).not.toBeInTheDocument();

      selectBuildType("last-beacon-windows-x64-test.tar.gz");

      expect(
        screen.queryByRole("button", { name: "Sync last-beacon-windows-x64-shipping.tar.gz" }),
      ).not.toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: "Sync last-beacon-windows-x64-test.tar.gz" }),
      ).toBeInTheDocument();
    });
  });

  it("shows a placeholder on the dropdown toggle when nothing is active", () => {
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag={null}
        activeAssetName={null}
        cachedAssetIds={new Set()}
        onSelect={noop}
        onCheck={noop}
        onDelete={noop}
      />,
    );

    selectBuildType("last-beacon-windows-x64-shipping.tar.gz");

    expect(screen.getByRole("button", { expanded: false })).toHaveTextContent("No active build");
  });

  it("shows the active build's release as the dropdown's collapsed value", () => {
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag="0.2.14"
        activeAssetName="last-beacon-windows-x64-shipping.tar.gz"
        cachedAssetIds={new Set()}
        onSelect={noop}
        onCheck={noop}
        onDelete={noop}
      />,
    );

    expect(screen.getByRole("button", { expanded: false })).toHaveTextContent("LastBeacon 0.2.14");
  });

  it("lists every release of the selected build type as an option once opened", () => {
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag={null}
        activeAssetName={null}
        cachedAssetIds={new Set()}
        onSelect={noop}
        onCheck={noop}
        onDelete={noop}
      />,
    );

    selectBuildType("last-beacon-windows-x64-shipping.tar.gz");
    expect(screen.queryByRole("button", { name: "LastBeacon 0.2.14" })).not.toBeInTheDocument();

    openDropdown();

    expect(screen.getByRole("button", { name: "LastBeacon 0.2.14" })).toBeInTheDocument();
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
        onSelect={noop}
        onCheck={noop}
        onDelete={noop}
      />,
    );

    openDropdown();

    const activeRow = screen.getByRole("button", { name: "LastBeacon 0.2.14" }).closest("li");
    expect(activeRow).toHaveTextContent("Active");
    const otherRow = screen.getByRole("button", { name: "LastBeacon 0.2.13" }).closest("li");
    expect(otherRow).not.toHaveTextContent("Active");
  });

  it("only shows a delete button for the selected type's asset when it's cached", () => {
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag={null}
        activeAssetName={null}
        cachedAssetIds={new Set([10])}
        onSelect={noop}
        onCheck={noop}
        onDelete={noop}
      />,
    );

    selectBuildType("last-beacon-windows-x64-shipping.tar.gz");
    openDropdown();
    expect(
      screen.getByRole("button", { name: "Delete downloaded last-beacon-windows-x64-shipping.tar.gz" }),
    ).toBeInTheDocument();

    selectBuildType("last-beacon-windows-x64-test.tar.gz");
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
        onSelect={noop}
        onCheck={noop}
        onDelete={noop}
      />,
    );

    selectBuildType("last-beacon-windows-x64-shipping.tar.gz");
    openDropdown();

    const syncButton = screen.getByRole("button", { name: "Sync last-beacon-windows-x64-shipping.tar.gz" });
    expect(syncButton).toHaveTextContent("Check");
  });

  it("calls onSelect (activating the build) when its option label is clicked", () => {
    const onSelect = vi.fn();
    const onCheck = vi.fn();
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag={null}
        activeAssetName={null}
        cachedAssetIds={new Set()}
        onSelect={onSelect}
        onCheck={onCheck}
        onDelete={noop}
      />,
    );

    selectBuildType("last-beacon-windows-x64-shipping.tar.gz");
    openDropdown();
    fireEvent.click(screen.getByRole("button", { name: "LastBeacon 0.2.14" }));

    expect(onSelect).toHaveBeenCalledWith(releases[0], 10);
    expect(onCheck).not.toHaveBeenCalled();
  });

  it("selecting an option does not itself change which build is shown as active", () => {
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag={null}
        activeAssetName={null}
        cachedAssetIds={new Set()}
        onSelect={noop}
        onCheck={noop}
        onDelete={noop}
      />,
    );

    selectBuildType("last-beacon-windows-x64-shipping.tar.gz");
    openDropdown();
    fireEvent.click(screen.getByRole("button", { name: "LastBeacon 0.2.14" }));

    expect(screen.getByRole("button", { expanded: true })).toHaveTextContent("No active build");
  });

  it("calls onCheck (not onSelect) when the Sync/Check button is clicked", () => {
    const onSelect = vi.fn();
    const onCheck = vi.fn();
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag={null}
        activeAssetName={null}
        cachedAssetIds={new Set([10])}
        onSelect={onSelect}
        onCheck={onCheck}
        onDelete={noop}
      />,
    );

    selectBuildType("last-beacon-windows-x64-shipping.tar.gz");
    openDropdown();
    fireEvent.click(screen.getByRole("button", { name: "Sync last-beacon-windows-x64-shipping.tar.gz" }));

    expect(onCheck).toHaveBeenCalledWith(releases[0], 10);
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("calls onDelete with the release and asset id when delete is clicked", () => {
    const onDelete = vi.fn();
    render(
      <ReleaseList
        releases={releases}
        activeReleaseTag={null}
        activeAssetName={null}
        cachedAssetIds={new Set([10])}
        onSelect={noop}
        onCheck={noop}
        onDelete={onDelete}
      />,
    );

    selectBuildType("last-beacon-windows-x64-shipping.tar.gz");
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
        onSelect={noop}
        onCheck={noop}
        onDelete={noop}
      />,
    );

    selectBuildType("last-beacon-windows-x64-shipping.tar.gz");
    openDropdown();
    expect(screen.getByRole("button", { name: "LastBeacon 0.2.14" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { expanded: true }));

    expect(screen.queryByRole("button", { name: "LastBeacon 0.2.14" })).not.toBeInTheDocument();
  });
});
