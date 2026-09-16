import { useCallback, useEffect, useState } from "react";
import { isLoggedIn, logout } from "./api/auth";
import { Login } from "./components/Login";
import "./App.css";

function App() {
  const [loggedIn, setLoggedIn] = useState<boolean | null>(null);

  useEffect(() => {
    isLoggedIn()
      .then(setLoggedIn)
      .catch(() => setLoggedIn(false));
  }, []);

  const handleLoggedIn = useCallback(() => setLoggedIn(true), []);

  if (loggedIn === null) {
    return <p>Loading...</p>;
  }

  if (!loggedIn) {
    return <Login onLoggedIn={handleLoggedIn} />;
  }

  return (
    <div>
      <p>Logged in.</p>
      <button
        onClick={async () => {
          await logout();
          setLoggedIn(false);
        }}
      >
        Log out
      </button>
    </div>
  );
}

export default App;
