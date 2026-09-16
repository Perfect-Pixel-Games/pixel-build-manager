import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { Login } from "./Login";
import * as authApi from "../api/auth";

vi.mock("../api/auth");

describe("Login", () => {
  beforeEach(() => {
    vi.mocked(authApi.loginStart).mockResolvedValue(undefined);
    vi.mocked(authApi.onLoginStatus).mockImplementation(() =>
      Promise.resolve(() => {}),
    );
  });

  it("shows the device code once the backend reports awaiting_user", async () => {
    let capturedCallback: (status: authApi.LoginStatus) => void = () => {};
    vi.mocked(authApi.onLoginStatus).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });

    render(<Login onLoggedIn={() => {}} />);

    // The listener is registered on mount (useEffect), so it's already in
    // place before the user has a chance to click the button.
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
});
