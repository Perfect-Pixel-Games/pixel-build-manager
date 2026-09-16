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

  it("transitions to done and resolves true after a successful sync", async () => {
    vi.mocked(syncApi.syncReleaseAsset).mockResolvedValue(undefined);
    const { result } = renderHook(() => useSync("org/repo"));

    let succeeded: boolean | undefined;
    await act(async () => {
      succeeded = await result.current.sync(release, asset);
    });

    expect(succeeded).toBe(true);
    expect(result.current.state).toEqual({ phase: "done" });
  });

  it("transitions to error and rethrows when the sync call rejects", async () => {
    vi.mocked(syncApi.syncReleaseAsset).mockRejectedValue(new Error("network down"));
    const { result } = renderHook(() => useSync("org/repo"));

    await act(async () => {
      await expect(result.current.sync(release, asset)).rejects.toThrow("network down");
    });

    expect(result.current.state).toEqual({ phase: "error", message: "Error: network down" });
  });

  it("transitions to done and resolves true after a successful check", async () => {
    vi.mocked(syncApi.checkReleaseAsset).mockResolvedValue(undefined);
    const { result } = renderHook(() => useSync("org/repo"));

    let succeeded: boolean | undefined;
    await act(async () => {
      succeeded = await result.current.check(asset);
    });

    expect(succeeded).toBe(true);
    expect(syncApi.checkReleaseAsset).toHaveBeenCalledWith("org/repo", asset);
    expect(result.current.state).toEqual({ phase: "done" });
  });

  it("transitions to error and rethrows when the check call rejects", async () => {
    vi.mocked(syncApi.checkReleaseAsset).mockRejectedValue(new Error("disk full"));
    const { result } = renderHook(() => useSync("org/repo"));

    await act(async () => {
      await expect(result.current.check(asset)).rejects.toThrow("disk full");
    });

    expect(result.current.state).toEqual({ phase: "error", message: "Error: disk full" });
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

  it("ignores a second sync() call while one is already in flight, resolving it to false", async () => {
    let resolveFirstSync: () => void = () => {};
    vi.mocked(syncApi.syncReleaseAsset).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveFirstSync = () => resolve(undefined);
        }),
    );
    const { result } = renderHook(() => useSync("org/repo"));

    let firstCall!: Promise<boolean>;
    let secondCall!: Promise<boolean>;
    act(() => {
      firstCall = result.current.sync(release, asset);
    });
    act(() => {
      // Second call while the first is still pending -- should be a no-op.
      secondCall = result.current.sync(release, asset);
    });

    expect(syncApi.syncReleaseAsset).toHaveBeenCalledTimes(1);
    await expect(secondCall).resolves.toBe(false);

    await act(async () => {
      resolveFirstSync();
    });

    await expect(firstCall).resolves.toBe(true);
    expect(result.current.state).toEqual({ phase: "done" });

    // Once the first call has finished, sync() should work again.
    vi.mocked(syncApi.syncReleaseAsset).mockResolvedValue(undefined);
    await act(async () => {
      await result.current.sync(release, asset);
    });
    expect(syncApi.syncReleaseAsset).toHaveBeenCalledTimes(2);
  });

  it("ignores a check() call while a sync() is already in flight (shared guard)", async () => {
    vi.mocked(syncApi.syncReleaseAsset).mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useSync("org/repo"));

    act(() => {
      result.current.sync(release, asset);
    });
    const checkResult = await act(async () => result.current.check(asset));

    expect(checkResult).toBe(false);
    expect(syncApi.checkReleaseAsset).not.toHaveBeenCalled();
  });
});
