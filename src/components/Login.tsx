import { useEffect, useState } from "react";
import { loginStart, onLoginStatus, LoginStatus } from "../api/auth";

type Props = {
  onLoggedIn: () => void;
};

export function Login({ onLoggedIn }: Props) {
  const [status, setStatus] = useState<LoginStatus | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    onLoginStatus((newStatus) => {
      setStatus(newStatus);
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
    try {
      await loginStart();
    } catch (error) {
      console.error("failed to start login", error);
    }
  };

  return (
    <div>
      <button onClick={handleLogin}>Log in with GitHub</button>
      {status?.status === "awaiting_user" && (
        <p>
          Go to {status.verification_uri} and enter code: <strong>{status.user_code}</strong>
        </p>
      )}
      {status?.status === "denied" && <p>Login was denied.</p>}
      {status?.status === "expired" && <p>The login code expired. Try again.</p>}
      {status?.status === "error" && <p>Login failed: {status.error}</p>}
    </div>
  );
}
