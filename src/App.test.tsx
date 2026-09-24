import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import App, { ProjectDetail } from "./App";
import * as projectsApi from "./api/projects";
import * as syncApi from "./api/sync";
import * as authApi from "./api/auth";
import { SESSION_EXPIRED_ERROR } from "./api/auth";
import * as versionApi from "./api/version";
import * as settingsApi from "./api/settings";
import type { Release } from "./api/projects";

vi.mock("./api/projects");
vi.mock("./api/sync");
vi.mock("./api/auth");
vi.mock("./api/version");
vi.mock("./api/settings");

const release: Release = {
  id: 1,
  tag_name: "0.2.14",
  name: "LastBeacon 0.2.14",
  prerelease: false,
  published_at: null,
  assets: [
    { id: 10, name: "shipping.zip", size: 100, browser_download_url: "https://example.com/a" },
    { id: 11, name: "test.zip", size: 100, browser_download_url: "https://example.com/b" },
  ],
};

function mockProjectDetailBaseline() {
  vi.mocked(projectsApi.listReleasesForProject).mockResolvedValue([release]);
  vi.mocked(syncApi.getSelectedRelease).mockResolvedValue(null);
  vi.mocked(syncApi.setSelectedRelease).mockResolvedValue(undefined);
  vi.mocked(syncApi.getTickedConfigs).mockResolvedValue([]);
  vi.mocked(syncApi.setTickedConfigs).mockResolvedValue(undefined);
  vi.mocked(syncApi.listSyncedConfigs).mockResolvedValue([]);
  vi.mocked(syncApi.onSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
  vi.mocked(syncApi.syncReleaseAsset).mockResolvedValue(undefined);
}

function mockAppShellBaseline() {
  vi.mocked(authApi.isLoggedIn).mockResolvedValue(true);
  vi.mocked(authApi.onLoginStatus).mockResolvedValue(() => {});
  vi.mocked(versionApi.getVersionLabel).mockResolvedValue("Release 0.4.0");
  vi.mocked(settingsApi.getWorkspaceRoot).mockResolvedValue("D:\\Builds");
  vi.mocked(settingsApi.getTheme).mockResolvedValue("system");
}

describe("ProjectDetail", () => {
  it("loads the persisted selected release and ticked configs, then renders them", async () => {
    mockProjectDetailBaseline();
    vi.mocked(syncApi.getSelectedRelease).mockResolvedValue("0.2.14");
    vi.mocked(syncApi.getTickedConfigs).mockResolvedValue(["shipping.zip"]);

    render(<ProjectDetail projectKey="org/repo" />);

    expect(await screen.findByRole("button", { name: "LastBeacon 0.2.14" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(await screen.findByLabelText("shipping.zip")).toBeChecked();
  });

  it("persists the selected release when a release row is clicked", async () => {
    mockProjectDetailBaseline();

    render(<ProjectDetail projectKey="org/repo" />);
    fireEvent.click(await screen.findByRole("button", { name: "LastBeacon 0.2.14" }));

    await waitFor(() => expect(syncApi.setSelectedRelease).toHaveBeenCalledWith("org/repo", "0.2.14"));
  });

  it("persists ticked configs and syncs only the missing ticked assets when Sync is clicked", async () => {
    mockProjectDetailBaseline();
    vi.mocked(syncApi.getSelectedRelease).mockResolvedValue("0.2.14");

    render(<ProjectDetail projectKey="org/repo" />);
    fireEvent.click(await screen.findByLabelText("shipping.zip"));

    await waitFor(() => expect(syncApi.setTickedConfigs).toHaveBeenCalledWith("org/repo", ["shipping.zip"]));

    fireEvent.click(screen.getByRole("button", { name: "Sync" }));

    await waitFor(() => expect(syncApi.syncReleaseAsset).toHaveBeenCalledTimes(1));
    expect(syncApi.syncReleaseAsset).toHaveBeenCalledWith("org/repo", release, release.assets[0]);
  });

  it("shows the busy overlay while a sync is in flight", async () => {
    mockProjectDetailBaseline();
    vi.mocked(syncApi.getSelectedRelease).mockResolvedValue("0.2.14");
    vi.mocked(syncApi.getTickedConfigs).mockResolvedValue(["shipping.zip"]);
    vi.mocked(syncApi.syncReleaseAsset).mockImplementation(() => new Promise(() => {}));

    render(<ProjectDetail projectKey="org/repo" />);
    fireEvent.click(await screen.findByRole("button", { name: "Sync" }));

    expect(await screen.findByRole("status")).toBeInTheDocument();
  });
});

describe("App", () => {
  it("shows the version label in the footer even before logging in", async () => {
    vi.mocked(authApi.isLoggedIn).mockResolvedValue(false);
    vi.mocked(authApi.onLoginStatus).mockResolvedValue(() => {});
    vi.mocked(versionApi.getVersionLabel).mockResolvedValue("Release 0.4.0");
    // useTheme() runs unconditionally (even pre-login, so the login screen
    // itself reflects the saved theme), so it must be mocked here too.
    vi.mocked(settingsApi.getTheme).mockResolvedValue("system");

    render(<App />);

    expect(await screen.findByText("Release 0.4.0")).toBeInTheDocument();
  });

  it("routes back to the Login screen when loading projects reports the session has expired", async () => {
    mockAppShellBaseline();
    vi.mocked(projectsApi.listProjects).mockRejectedValue(SESSION_EXPIRED_ERROR);

    render(<App />);

    expect(await screen.findByRole("button", { name: /log in with github/i })).toBeInTheDocument();
  });

  it("shows only a + button when no projects are bound", async () => {
    mockAppShellBaseline();
    vi.mocked(projectsApi.listProjects).mockResolvedValue([
      { full_name: "org/repo", owner: "org", name: "repo", favorite: false },
    ]);
    vi.mocked(settingsApi.listBoundProjects).mockResolvedValue([]);

    render(<App />);

    expect(await screen.findByRole("button", { name: "Bind a project" })).toBeInTheDocument();
    expect(screen.queryByRole("tab")).not.toBeInTheDocument();
  });

  it("binding a project via the popup opens its tab", async () => {
    mockAppShellBaseline();
    mockProjectDetailBaseline();
    vi.mocked(projectsApi.listProjects).mockResolvedValue([
      { full_name: "org/repo", owner: "org", name: "repo", favorite: false },
    ]);
    vi.mocked(settingsApi.listBoundProjects).mockResolvedValue([]);
    vi.mocked(settingsApi.bindProject).mockResolvedValue(undefined);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Bind a project" }));
    fireEvent.click(await screen.findByRole("button", { name: "repo" }));

    await waitFor(() => expect(settingsApi.bindProject).toHaveBeenCalledWith("org/repo"));
    expect(await screen.findByRole("tab", { selected: true })).toHaveTextContent("repo");
  });

  it("closing a tab's x unbinds the project", async () => {
    mockAppShellBaseline();
    mockProjectDetailBaseline();
    vi.mocked(projectsApi.listProjects).mockResolvedValue([
      { full_name: "org/repo", owner: "org", name: "repo", favorite: false },
    ]);
    vi.mocked(settingsApi.listBoundProjects).mockResolvedValue(["org/repo"]);
    vi.mocked(settingsApi.unbindProject).mockResolvedValue(undefined);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Close repo" }));

    await waitFor(() => expect(settingsApi.unbindProject).toHaveBeenCalledWith("org/repo"));
    expect(screen.queryByRole("tab")).not.toBeInTheDocument();
  });
});
