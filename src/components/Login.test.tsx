import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { Login } from "./Login";
import * as authApi from "../api/auth";
import * as opener from "@tauri-apps/plugin-opener";

vi.mock("../api/auth");
vi.mock("@tauri-apps/plugin-opener");

describe("Login", () => {
  beforeEach(() => {
    vi.mocked(authApi.loginStart).mockResolvedValue(undefined);
    vi.mocked(authApi.onLoginStatus).mockImplementation(() => Promise.resolve(() => {}));
    vi.mocked(opener.openUrl).mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
    });
  });

  it("shows the device code once the backend reports awaiting_user", async () => {
    let capturedCallback: (status: authApi.LoginStatus) => void = () => {};
    vi.mocked(authApi.onLoginStatus).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });

    render(<Login onLoggedIn={() => {}} />);

    await waitFor(() => expect(authApi.onLoginStatus).toHaveBeenCalled());

    fireEvent.click(screen.getByRole("button", { name: /log in with github/i }));
    await waitFor(() => expect(authApi.loginStart).toHaveBeenCalled());

    capturedCallback({
      status: "awaiting_user",
      user_code: "ABCD-1234",
      verification_uri: "https://github.com/login/device",
    });

    expect(await screen.findByText("ABCD-1234")).toBeInTheDocument();
  });

  it("calls onLoggedIn when the backend reports success", async () => {
    let capturedCallback: (status: authApi.LoginStatus) => void = () => {};
    vi.mocked(authApi.onLoginStatus).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });
    const onLoggedIn = vi.fn();

    render(<Login onLoggedIn={onLoggedIn} />);
    await waitFor(() => expect(authApi.onLoginStatus).toHaveBeenCalled());

    fireEvent.click(screen.getByRole("button", { name: /log in with github/i }));
    await waitFor(() => expect(authApi.loginStart).toHaveBeenCalled());

    capturedCallback({ status: "success" });

    await waitFor(() => expect(onLoggedIn).toHaveBeenCalled());
  });

  it("shows a message when the backend reports denied", async () => {
    let capturedCallback: (status: authApi.LoginStatus) => void = () => {};
    vi.mocked(authApi.onLoginStatus).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });

    render(<Login onLoggedIn={() => {}} />);
    await waitFor(() => expect(authApi.onLoginStatus).toHaveBeenCalled());

    capturedCallback({ status: "denied" });

    expect(await screen.findByText(/login was denied/i)).toBeInTheDocument();
  });

  it("shows the error message when the backend reports an error", async () => {
    let capturedCallback: (status: authApi.LoginStatus) => void = () => {};
    vi.mocked(authApi.onLoginStatus).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });

    render(<Login onLoggedIn={() => {}} />);
    await waitFor(() => expect(authApi.onLoginStatus).toHaveBeenCalled());

    capturedCallback({ status: "error", error: "network unreachable" });

    expect(await screen.findByText(/login failed: network unreachable/i)).toBeInTheDocument();
  });

  it("automatically opens the verification link once the device code arrives", async () => {
    let capturedCallback: (status: authApi.LoginStatus) => void = () => {};
    vi.mocked(authApi.onLoginStatus).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });

    render(<Login onLoggedIn={() => {}} />);
    await waitFor(() => expect(authApi.onLoginStatus).toHaveBeenCalled());

    capturedCallback({
      status: "awaiting_user",
      user_code: "ABCD-1234",
      verification_uri: "https://github.com/login/device",
    });

    await waitFor(() => expect(opener.openUrl).toHaveBeenCalledWith("https://github.com/login/device"));
  });

  it("copies the device code to the clipboard when Copy is clicked", async () => {
    let capturedCallback: (status: authApi.LoginStatus) => void = () => {};
    vi.mocked(authApi.onLoginStatus).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });

    render(<Login onLoggedIn={() => {}} />);
    await waitFor(() => expect(authApi.onLoginStatus).toHaveBeenCalled());

    capturedCallback({
      status: "awaiting_user",
      user_code: "ABCD-1234",
      verification_uri: "https://github.com/login/device",
    });

    fireEvent.click(await screen.findByRole("button", { name: /^copy$/i }));

    await waitFor(() => expect(navigator.clipboard.writeText).toHaveBeenCalledWith("ABCD-1234"));
    expect(await screen.findByRole("button", { name: /copied/i })).toBeInTheDocument();
  });

  it("still offers a manual fallback link to the verification URL", async () => {
    let capturedCallback: (status: authApi.LoginStatus) => void = () => {};
    vi.mocked(authApi.onLoginStatus).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });

    render(<Login onLoggedIn={() => {}} />);
    await waitFor(() => expect(authApi.onLoginStatus).toHaveBeenCalled());

    capturedCallback({
      status: "awaiting_user",
      user_code: "ABCD-1234",
      verification_uri: "https://github.com/login/device",
    });

    fireEvent.click(await screen.findByRole("link", { name: /open.*manually/i }));

    await waitFor(() => expect(opener.openUrl).toHaveBeenCalledWith("https://github.com/login/device"));
  });
});
