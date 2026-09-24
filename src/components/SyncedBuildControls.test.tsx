import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SyncedBuildControls } from "./SyncedBuildControls";
import * as syncApi from "../api/sync";
import * as opener from "@tauri-apps/plugin-opener";

vi.mock("../api/sync");
vi.mock("@tauri-apps/plugin-opener");

describe("SyncedBuildControls", () => {
  beforeEach(() => {
    vi.mocked(opener.openPath).mockResolvedValue(undefined);
  });

  it("renders both buttons once the build dir and executable resolve", async () => {
    vi.mocked(syncApi.getBuildDir).mockResolvedValue("D:\\Builds\\org\\repo\\builds\\0.2.14\\shipping.zip");
    vi.mocked(syncApi.getBuildExecutable).mockResolvedValue(
      "D:\\Builds\\org\\repo\\builds\\0.2.14\\shipping.zip\\game.exe",
    );

    render(
      <SyncedBuildControls
        projectKey="org/repo"
        releaseTag="0.2.14"
        configName="shipping.zip"
        disabled={false}
      />,
    );

    expect(await screen.findByRole("button", { name: "Open folder for shipping.zip" })).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Launch shipping.zip" })).toBeInTheDocument();
  });

  it("renders neither button when no executable or dir is found yet", async () => {
    vi.mocked(syncApi.getBuildDir).mockResolvedValue(null);
    vi.mocked(syncApi.getBuildExecutable).mockResolvedValue(null);

    render(
      <SyncedBuildControls
        projectKey="org/repo"
        releaseTag="0.2.14"
        configName="shipping.zip"
        disabled={false}
      />,
    );

    await waitFor(() => expect(syncApi.getBuildDir).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: /open folder/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /launch/i })).not.toBeInTheDocument();
  });

  it("opens the build dir when Open Folder is clicked", async () => {
    vi.mocked(syncApi.getBuildDir).mockResolvedValue("D:\\Builds\\shipping.zip");
    vi.mocked(syncApi.getBuildExecutable).mockResolvedValue(null);

    render(
      <SyncedBuildControls
        projectKey="org/repo"
        releaseTag="0.2.14"
        configName="shipping.zip"
        disabled={false}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Open folder for shipping.zip" }));

    await waitFor(() => expect(opener.openPath).toHaveBeenCalledWith("D:\\Builds\\shipping.zip"));
  });

  it("launches the build when Launch is clicked", async () => {
    vi.mocked(syncApi.getBuildDir).mockResolvedValue(null);
    vi.mocked(syncApi.getBuildExecutable).mockResolvedValue("D:\\Builds\\shipping.zip\\game.exe");
    vi.mocked(syncApi.launchBuild).mockResolvedValue(undefined);

    render(
      <SyncedBuildControls
        projectKey="org/repo"
        releaseTag="0.2.14"
        configName="shipping.zip"
        disabled={false}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Launch shipping.zip" }));

    await waitFor(() =>
      expect(syncApi.launchBuild).toHaveBeenCalledWith("org/repo", "0.2.14", "shipping.zip"),
    );
  });

  it("shows an error message when launching fails", async () => {
    vi.mocked(syncApi.getBuildDir).mockResolvedValue(null);
    vi.mocked(syncApi.getBuildExecutable).mockResolvedValue("D:\\Builds\\shipping.zip\\game.exe");
    vi.mocked(syncApi.launchBuild).mockRejectedValue(new Error("exe not found"));

    render(
      <SyncedBuildControls
        projectKey="org/repo"
        releaseTag="0.2.14"
        configName="shipping.zip"
        disabled={false}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Launch shipping.zip" }));

    expect(await screen.findByText("exe not found")).toBeInTheDocument();
  });

  it("disables both buttons when disabled is true", async () => {
    vi.mocked(syncApi.getBuildDir).mockResolvedValue("D:\\Builds\\shipping.zip");
    vi.mocked(syncApi.getBuildExecutable).mockResolvedValue("D:\\Builds\\shipping.zip\\game.exe");

    render(
      <SyncedBuildControls
        projectKey="org/repo"
        releaseTag="0.2.14"
        configName="shipping.zip"
        disabled
      />,
    );

    expect(await screen.findByRole("button", { name: "Open folder for shipping.zip" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Launch shipping.zip" })).toBeDisabled();
  });
});
