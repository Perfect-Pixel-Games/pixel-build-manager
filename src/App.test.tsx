import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import App, { ProjectDetail } from "./App";
import * as projectsApi from "./api/projects";
import * as syncApi from "./api/sync";
import * as authApi from "./api/auth";
import * as versionApi from "./api/version";
import type { Release } from "./api/projects";

vi.mock("./api/projects");
vi.mock("./api/sync");
vi.mock("./api/auth");
vi.mock("./api/version");

const release: Release = {
  id: 1,
  tag_name: "0.2.14",
  name: "LastBeacon 0.2.14",
  prerelease: false,
  published_at: null,
  assets: [
    { id: 10, name: "shipping.zip", size: 100, browser_download_url: "https://example.com/a" },
    { id: 11, name: "test.zip", size: 100, browser_download_url: "https://example.com/b" },
    { id: 12, name: "ps4.zip", size: 100, browser_download_url: "https://example.com/c" },
  ],
};

function mockBaseline() {
  vi.mocked(projectsApi.listReleasesForProject).mockResolvedValue([release]);
  vi.mocked(syncApi.getActiveRelease).mockResolvedValue({ release_tag: null, asset_name: null });
  vi.mocked(syncApi.getActiveExecutable).mockResolvedValue(null);
  vi.mocked(syncApi.getActiveBuildDir).mockResolvedValue(null);
  vi.mocked(syncApi.listCachedAssets).mockResolvedValue([]);
  vi.mocked(syncApi.onSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
  vi.mocked(syncApi.checkReleaseAsset).mockResolvedValue(undefined);
  vi.mocked(syncApi.syncReleaseAsset).mockResolvedValue(undefined);
}

describe("ProjectDetail", () => {
  it("downloads every other build type of a release when one is selected, and only activates the chosen one", async () => {
    mockBaseline();

    render(<ProjectDetail projectKey="org/repo" />);

    fireEvent.change(await screen.findByLabelText("Build type"), { target: { value: "shipping.zip" } });
    fireEvent.click(screen.getByRole("button", { expanded: false }));
    fireEvent.click(screen.getByRole("button", { name: "LastBeacon 0.2.14" }));

    await waitFor(() => expect(syncApi.syncReleaseAsset).toHaveBeenCalled());

    expect(syncApi.checkReleaseAsset).toHaveBeenCalledTimes(2);
    expect(syncApi.checkReleaseAsset).toHaveBeenNthCalledWith(1, "org/repo", release.assets[1]);
    expect(syncApi.checkReleaseAsset).toHaveBeenNthCalledWith(2, "org/repo", release.assets[2]);
    expect(syncApi.syncReleaseAsset).toHaveBeenCalledWith("org/repo", release, release.assets[0]);
  });
});

describe("App", () => {
  it("shows the version label in the footer even before logging in", async () => {
    vi.mocked(authApi.isLoggedIn).mockResolvedValue(false);
    vi.mocked(authApi.onLoginStatus).mockResolvedValue(() => {});
    vi.mocked(versionApi.getVersionLabel).mockResolvedValue("Release 0.4.0");

    render(<App />);

    expect(await screen.findByText("Release 0.4.0")).toBeInTheDocument();
  });
});
