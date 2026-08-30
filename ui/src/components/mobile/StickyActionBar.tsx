import type { ReactNode } from "react";

interface StickyActionBarProps {
  children: ReactNode;
  /** Optional aria-label for the action region */
  ariaLabel?: string;
}

export function StickyActionBar({ children, ariaLabel }: StickyActionBarProps) {
  return (
    <div className="mobile-sticky-bar" role={ariaLabel ? "toolbar" : undefined} aria-label={ariaLabel}>
      {children}
    </div>
  );
}

export default StickyActionBar;
