import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ClearCacheButton } from "./ClearCacheButton";
import * as syncApi from "../api/sync";

vi.mock("../api/sync");

describe("ClearCacheButton", () => {
  it("clears the cache for the given project", async () => {
    vi.mocked(syncApi.clearProjectCache).mockResolvedValue(undefined);

    render(<ClearCacheButton projectKey="owner/repo" />);
    fireEvent.click(screen.getByRole("button", { name: /clear cache/i }));

    await waitFor(() => expect(syncApi.clearProjectCache).toHaveBeenCalledWith("owner/repo"));
  });

  it("calls onCleared after a successful clear", async () => {
    vi.mocked(syncApi.clearProjectCache).mockResolvedValue(undefined);
    const onCleared = vi.fn();

    render(<ClearCacheButton projectKey="owner/repo" onCleared={onCleared} />);
    fireEvent.click(screen.getByRole("button", { name: /clear cache/i }));

    await waitFor(() => expect(onCleared).toHaveBeenCalled());
  });

  it("does not call onCleared when clearing the cache fails", async () => {
    vi.mocked(syncApi.clearProjectCache).mockRejectedValue(new Error("disk error"));
    const onCleared = vi.fn();

    render(<ClearCacheButton projectKey="owner/repo" onCleared={onCleared} />);
    fireEvent.click(screen.getByRole("button", { name: /clear cache/i }));

    expect(await screen.findByText(/failed to clear cache: disk error/i)).toBeInTheDocument();
    expect(onCleared).not.toHaveBeenCalled();
  });

  it("shows an error message when clearing the cache fails", async () => {
    vi.mocked(syncApi.clearProjectCache).mockRejectedValue(new Error("disk error"));

    render(<ClearCacheButton projectKey="owner/repo" />);
    fireEvent.click(screen.getByRole("button", { name: /clear cache/i }));

    expect(await screen.findByText(/failed to clear cache: disk error/i)).toBeInTheDocument();
  });

  it("is disabled when the disabled prop is true, even though nothing is pending", () => {
    render(<ClearCacheButton projectKey="owner/repo" disabled />);

    expect(screen.getByRole("button", { name: /clear cache/i })).toBeDisabled();
  });

  it("clears the previous error on the next click attempt", async () => {
    vi.mocked(syncApi.clearProjectCache)
      .mockRejectedValueOnce(new Error("disk error"))
      .mockResolvedValueOnce(undefined);

    render(<ClearCacheButton projectKey="owner/repo" />);
    fireEvent.click(screen.getByRole("button", { name: /clear cache/i }));
    expect(await screen.findByText(/failed to clear cache: disk error/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /clear cache/i }));

    await waitFor(() =>
      expect(screen.queryByText(/failed to clear cache: disk error/i)).not.toBeInTheDocument(),
    );
  });
});
