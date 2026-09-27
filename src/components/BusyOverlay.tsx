import type { ReactNode } from "react";

type Props = {
  active: boolean;
  label?: string;
  children: ReactNode;
};

/** Wraps `children` in a dimmed, inert layer with a centered throbber +
 * label overlaid on top whenever `active` -- the wrapped UI stays mounted
 * and visible underneath rather than being replaced. */
export function BusyOverlay({ active, label, children }: Props) {
  return (
    <div className="busy-overlay-container">
      <div
        className={`busy-overlay-content${active ? " busy-overlay-content--dimmed" : ""}`}
        inert={active}
      >
        {children}
      </div>
      {active && (
        <div className="busy-overlay-layer" role="status">
          <span className="throbber" aria-hidden="true" />
          {label && <span>{label}</span>}
        </div>
      )}
    </div>
  );
}
