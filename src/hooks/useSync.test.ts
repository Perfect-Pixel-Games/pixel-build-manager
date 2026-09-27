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
const shippingAsset: ReleaseAsset = {
  id: 10,
  name: "shipping.zip",
  size: 1000,
  browser_download_url: "https://example.com/shipping.zip",
};
const testAsset: ReleaseAsset = {
  id: 11,
  name: "test.zip",
  size: 500,
  browser_download_url: "https://example.com/test.zip",
};

describe("useSync", () => {
  beforeEach(() => {
    vi.mocked(syncApi.onSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
  });

  it("syncs a single asset and transitions to done", async () => {
    vi.mocked(syncApi.syncReleaseAsset).mockResolvedValue(undefined);
    const { result } = renderHook(() => useSync("org/repo"));

    let succeeded: boolean | undefined;
    await act(async () => {
      succeeded = await result.current.syncConfigs(release, [shippingAsset]);
    });

    expect(succeeded).toBe(true);
    expect(result.current.state).toEqual({ phase: "done" });
    expect(syncApi.syncReleaseAsset).toHaveBeenCalledWith("org/repo", release, shippingAsset);
  });

  it("syncs multiple assets one at a time, in order", async () => {
    const callOrder: string[] = [];
    vi.mocked(syncApi.syncReleaseAsset).mockImplementation(async (_projectKey, _release, asset) => {
      callOrder.push(asset.name);
    });
    const { result } = renderHook(() => useSync("org/repo"));

    await act(async () => {
      await result.current.syncConfigs(release, [shippingAsset, testAsset]);
    });

    expect(callOrder).toEqual(["shipping.zip", "test.zip"]);
  });

  it("reports which config is currently syncing", async () => {
    let resolveShipping: () => void = () => {};
    vi.mocked(syncApi.syncReleaseAsset).mockImplementation(
      () => new Promise((resolve) => (resolveShipping = () => resolve(undefined))),
    );
    const { result } = renderHook(() => useSync("org/repo"));

    act(() => {
      result.current.syncConfigs(release, [shippingAsset]);
    });

    expect(result.current.state).toEqual({
      phase: "syncing",
      configName: "shipping.zip",
      downloaded: 0,
      total: 1000,
    });

    await act(async () => resolveShipping());
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
      result.current.syncConfigs(release, [shippingAsset]);
    });
    act(() => {
      capturedCallback({ project_key: "org/other-repo", downloaded: 5, total: 1000 });
    });
    expect(result.current.state).toMatchObject({ downloaded: 0, total: 1000 });

    act(() => {
      capturedCallback({ project_key: "org/repo", downloaded: 500, total: 1000 });
    });
    await waitFor(() =>
      expect(result.current.state).toMatchObject({ downloaded: 500, total: 1000 }),
    );
  });

  it("transitions to error and rethrows when a sync call rejects, without syncing the rest of the batch", async () => {
    vi.mocked(syncApi.syncReleaseAsset).mockRejectedValue(new Error("network down"));
    const { result } = renderHook(() => useSync("org/repo"));

    await act(async () => {
      await expect(result.current.syncConfigs(release, [shippingAsset, testAsset])).rejects.toThrow(
        "network down",
      );
    });

    expect(result.current.state).toEqual({ phase: "error", message: "Error: network down" });
    expect(syncApi.syncReleaseAsset).toHaveBeenCalledTimes(1);
  });

  it("ignores a second syncConfigs() call while one is already in flight, resolving it to false", async () => {
    let resolveFirstSync: () => void = () => {};
    vi.mocked(syncApi.syncReleaseAsset).mockImplementation(
      () => new Promise((resolve) => (resolveFirstSync = () => resolve(undefined))),
    );
    const { result } = renderHook(() => useSync("org/repo"));

    let firstCall!: Promise<boolean>;
    let secondCall!: Promise<boolean>;
    act(() => {
      firstCall = result.current.syncConfigs(release, [shippingAsset]);
    });
    act(() => {
      secondCall = result.current.syncConfigs(release, [testAsset]);
    });

    expect(syncApi.syncReleaseAsset).toHaveBeenCalledTimes(1);
    await expect(secondCall).resolves.toBe(false);

    await act(async () => resolveFirstSync());

    await expect(firstCall).resolves.toBe(true);
    expect(result.current.state).toEqual({ phase: "done" });
  });
});
