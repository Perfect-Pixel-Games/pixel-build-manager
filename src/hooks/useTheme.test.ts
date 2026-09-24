import { describe, expect, it, vi } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { useTheme } from "./useTheme";
import * as settingsApi from "../api/settings";

vi.mock("../api/settings");

describe("useTheme", () => {
  it("loads the persisted theme on mount and applies it to the root element", async () => {
    vi.mocked(settingsApi.getTheme).mockResolvedValue("dark");

    const { result } = renderHook(() => useTheme());

    await waitFor(() => expect(result.current.theme).toBe("dark"));
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });

  it("removes the data-theme attribute for system so the media query decides", async () => {
    vi.mocked(settingsApi.getTheme).mockResolvedValue("system");
    document.documentElement.setAttribute("data-theme", "dark");

    renderHook(() => useTheme());

    await waitFor(() =>
      expect(document.documentElement.hasAttribute("data-theme")).toBe(false),
    );
  });

  it("persists and applies a new theme when setTheme is called", async () => {
    vi.mocked(settingsApi.getTheme).mockResolvedValue("system");
    vi.mocked(settingsApi.setTheme).mockResolvedValue(undefined);
    const { result } = renderHook(() => useTheme());
    await waitFor(() => expect(result.current.theme).toBe("system"));

    act(() => {
      result.current.setTheme("light");
    });

    expect(result.current.theme).toBe("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(settingsApi.setTheme).toHaveBeenCalledWith("light");
  });
});
