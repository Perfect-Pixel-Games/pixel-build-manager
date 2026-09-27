import { describe, expect, it, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useLatestSync } from "./useLatestSync";
import * as syncApi from "../api/sync";

vi.mock("../api/sync");

describe("useLatestSync", () => {
  it("starts idle", () => {
    vi.mocked(syncApi.onLatestSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
    vi.mocked(syncApi.onLatestSyncFinished).mockImplementation(() => Promise.resolve(() => {}));

    const { result } = renderHook(() => useLatestSync("org/repo"));

    expect(result.current.state).toEqual({ phase: "idle" });
  });

  it("transitions to syncing on a matching progress event", () => {
    let capturedProgress: (p: syncApi.LatestSyncProgress) => void = () => {};
    vi.mocked(syncApi.onLatestSyncProgress).mockImplementation((cb) => {
      capturedProgress = cb;
      return Promise.resolve(() => {});
    });
    vi.mocked(syncApi.onLatestSyncFinished).mockImplementation(() => Promise.resolve(() => {}));
    const { result } = renderHook(() => useLatestSync("org/repo"));

    act(() => {
      capturedProgress({ project_key: "org/repo", channel: "release", downloaded: 5, total: 10 });
    });

    expect(result.current.state).toEqual({
      phase: "syncing",
      channel: "release",
      downloaded: 5,
      total: 10,
    });
  });

  it("ignores progress events for a different project", () => {
    let capturedProgress: (p: syncApi.LatestSyncProgress) => void = () => {};
    vi.mocked(syncApi.onLatestSyncProgress).mockImplementation((cb) => {
      capturedProgress = cb;
      return Promise.resolve(() => {});
    });
    vi.mocked(syncApi.onLatestSyncFinished).mockImplementation(() => Promise.resolve(() => {}));
    const { result } = renderHook(() => useLatestSync("org/repo"));

    act(() => {
      capturedProgress({ project_key: "org/other-repo", channel: "release", downloaded: 5, total: 10 });
    });

    expect(result.current.state).toEqual({ phase: "idle" });
  });

  it("transitions to done on a successful finished event", () => {
    vi.mocked(syncApi.onLatestSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
    let capturedFinished: (r: syncApi.LatestSyncFinished) => void = () => {};
    vi.mocked(syncApi.onLatestSyncFinished).mockImplementation((cb) => {
      capturedFinished = cb;
      return Promise.resolve(() => {});
    });
    const { result } = renderHook(() => useLatestSync("org/repo"));

    act(() => {
      capturedFinished({ project_key: "org/repo", channel: "release", synced: true, tag: "0.2.14", error: null });
    });

    expect(result.current.state).toEqual({ phase: "done", channel: "release", synced: true, tag: "0.2.14" });
  });

  it("transitions to error when the finished event carries an error", () => {
    vi.mocked(syncApi.onLatestSyncProgress).mockImplementation(() => Promise.resolve(() => {}));
    let capturedFinished: (r: syncApi.LatestSyncFinished) => void = () => {};
    vi.mocked(syncApi.onLatestSyncFinished).mockImplementation((cb) => {
      capturedFinished = cb;
      return Promise.resolve(() => {});
    });
    const { result } = renderHook(() => useLatestSync("org/repo"));

    act(() => {
      capturedFinished({
        project_key: "org/repo",
        channel: "release",
        synced: false,
        tag: null,
        error: "network down",
      });
    });

    expect(result.current.state).toEqual({ phase: "error", channel: "release", message: "network down" });
  });
});
