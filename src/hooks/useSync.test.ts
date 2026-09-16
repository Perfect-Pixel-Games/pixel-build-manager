import { describe, expect, it, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useSync } from "./useSync";
import * as syncApi from "../api/sync";
import type { Release, ReleaseAsset } from "../api/projects";

vi.mock("../api/sync");

const release: Release = {
  id: 1,
  tag_name: "0.2.14",
  name: "0.2.14",
  prerelease: false,
  published_at: null,
  assets: [],
};
const asset: ReleaseAsset = {
  id: 10,
  name: "build.zip",
  size: 1000,
  browser_download_url: "https://example.com/build.zip",
};

describe("useSync", () => {
  beforeEach(() => {
    vi.mocked(syncApi.onSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
  });

  it("transitions to done after a successful sync", async () => {
    vi.mocked(syncApi.syncReleaseAsset).mockResolvedValue(undefined);
    const { result } = renderHook(() => useSync("org/repo"));

    await act(async () => {
      await result.current.sync(release, asset);
    });

    expect(result.current.state).toEqual({ phase: "done" });
  });

  it("transitions to error when the sync call rejects", async () => {
    vi.mocked(syncApi.syncReleaseAsset).mockRejectedValue(new Error("network down"));
    const { result } = renderHook(() => useSync("org/repo"));

    await act(async () => {
      await result.current.sync(release, asset);
    });

    expect(result.current.state).toEqual({ phase: "error", message: "Error: network down" });
  });

  it("updates progress only for matching project_key events", async () => {
    let capturedCallback: (progress: syncApi.SyncProgress) => void = () => {};
    vi.mocked(syncApi.onSyncProgress).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });
    vi.mocked(syncApi.syncReleaseAsset).mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useSync("org/repo"));

    act(() => {
      result.current.sync(release, asset);
    });
    act(() => {
      capturedCallback({ project_key: "org/other-repo", downloaded: 5, total: 1000 });
    });
    expect(result.current.state).toEqual({ phase: "syncing", downloaded: 0, total: 1000 });

    act(() => {
      capturedCallback({ project_key: "org/repo", downloaded: 500, total: 1000 });
    });
    await waitFor(() =>
      expect(result.current.state).toEqual({ phase: "syncing", downloaded: 500, total: 1000 }),
    );
  });
});
