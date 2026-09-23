import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

// @testing-library/react's auto-cleanup relies on a global `afterEach`,
// which vitest does not inject unless `test.globals: true` is set. Register
// it explicitly so component trees are unmounted between tests.
afterEach(() => {
  cleanup();
  // useTheme sets this directly on <html>, outside the React tree cleanup()
  // unmounts -- reset it so one test's theme choice can't leak into the next.
  document.documentElement.removeAttribute("data-theme");
});
