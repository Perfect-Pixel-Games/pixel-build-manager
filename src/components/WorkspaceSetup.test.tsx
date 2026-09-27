import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { WorkspaceSetup } from "./WorkspaceSetup";
import * as settingsApi from "../api/settings";

vi.mock("../api/settings");

describe("WorkspaceSetup", () => {
  it("saves the picked folder and reports it back", async () => {
    vi.mocked(settingsApi.pickFolder).mockResolvedValue("D:\\Builds");
    vi.mocked(settingsApi.setWorkspaceRoot).mockResolvedValue(undefined);
    const onSet = vi.fn();

    render(<WorkspaceSetup onSet={onSet} />);
    fireEvent.click(screen.getByRole("button", { name: /choose workspace folder/i }));

    await waitFor(() => expect(settingsApi.setWorkspaceRoot).toHaveBeenCalledWith("D:\\Builds"));
    expect(onSet).toHaveBeenCalledWith("D:\\Builds");
  });

  it("does nothing if the user cancels the folder picker", async () => {
    vi.mocked(settingsApi.pickFolder).mockResolvedValue(null);
    const onSet = vi.fn();

    render(<WorkspaceSetup onSet={onSet} />);
    fireEvent.click(screen.getByRole("button", { name: /choose workspace folder/i }));

    await waitFor(() => expect(settingsApi.pickFolder).toHaveBeenCalled());
    expect(onSet).not.toHaveBeenCalled();
  });
});
