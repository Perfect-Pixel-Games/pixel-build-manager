import type { ReactNode } from "react";
import type { Theme } from "../hooks/useTheme";
import { MoonIcon, SunIcon, SystemIcon } from "./icons";

type Props = {
  theme: Theme;
  onChange: (theme: Theme) => void;
};

const OPTIONS: { value: Theme; label: string; icon: ReactNode }[] = [
  { value: "light", label: "Light", icon: <SunIcon /> },
  { value: "system", label: "System", icon: <SystemIcon /> },
  { value: "dark", label: "Dark", icon: <MoonIcon /> },
];

export function ThemeToggle({ theme, onChange }: Props) {
  return (
    <div className="theme-toggle" role="group" aria-label="Theme">
      {OPTIONS.map((option) => (
        <button
          key={option.value}
          aria-label={option.label}
          title={option.label}
          aria-pressed={theme === option.value}
          onClick={() => onChange(option.value)}
        >
          {option.icon}
        </button>
      ))}
    </div>
  );
}
