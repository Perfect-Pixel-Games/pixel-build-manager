import { useEffect, useState } from "react";
import { getTheme, setTheme as persistTheme, type Theme } from "../api/settings";

export type { Theme };

/** Loads the persisted theme on mount, applies it to <html data-theme>, and
 * persists any change the caller makes via the returned setter. "system"
 * removes the attribute entirely so `@media (prefers-color-scheme)` decides. */
export function useTheme() {
  const [theme, setThemeState] = useState<Theme>("system");

  useEffect(() => {
    getTheme()
      .then(setThemeState)
      .catch((error) => console.error("failed to load theme", error));
  }, []);

  useEffect(() => {
    if (theme === "system") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", theme);
    }
  }, [theme]);

  const setTheme = (next: Theme) => {
    setThemeState(next);
    persistTheme(next).catch((error) => console.error("failed to save theme", error));
  };

  return { theme, setTheme };
}
