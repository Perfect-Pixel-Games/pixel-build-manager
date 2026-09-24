import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { loginStart, onLoginStatus, LoginStatus } from "../api/auth";

type Props = {
  onLoggedIn: () => void;
};

export function Login({ onLoggedIn }: Props) {
  const [status, setStatus] = useState<LoginStatus | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    onLoginStatus((newStatus) => {
      setStatus(newStatus);
      if (newStatus.status === "awaiting_user") {
        openUrl(newStatus.verification_uri).catch((error) =>
          console.error("failed to auto-open the verification link", error),
        );
      }
      if (newStatus.status === "success") {
        onLoggedIn();
      }
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [onLoggedIn]);

  const handleLogin = async () => {
    setStatus(null);
    setCopied(false);
    try {
      await loginStart();
    } catch (error) {
      console.error("failed to start login", error);
    }
  };

  const handleCopyCode = async (code: string) => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
    } catch (error) {
      console.error("failed to copy the device code", error);
    }
  };

  const handleOpenManually = (verificationUri: string) => {
    openUrl(verificationUri).catch((error) =>
      console.error("failed to open the verification link", error),
    );
  };

  return (
    <div className="centered-screen">
      <div className="centered-card">
        <p className="centered-card__lede">Sign in to browse and sync your team's builds.</p>
        <button onClick={handleLogin}>Log in with GitHub</button>
        {status?.status === "awaiting_user" && (
          <div className="login-device">
            <p>We opened {status.verification_uri} in your browser.</p>
            <span className="device-code">
              <strong>{status.user_code}</strong>
              <button onClick={() => handleCopyCode(status.user_code)}>{copied ? "Copied!" : "Copy"}</button>
            </span>
            <p className="login-device__fallback">
              Didn't open automatically?{" "}
              <a
                href={status.verification_uri}
                onClick={(event) => {
                  event.preventDefault();
                  handleOpenManually(status.verification_uri);
                }}
              >
                Open {status.verification_uri} manually
              </a>
            </p>
          </div>
        )}
        {status?.status === "denied" && <p className="error-text">Login was denied.</p>}
        {status?.status === "expired" && <p className="error-text">The login code expired. Try again.</p>}
        {status?.status === "error" && <p className="error-text">Login failed: {status.error}</p>}
      </div>
    </div>
  );
}
