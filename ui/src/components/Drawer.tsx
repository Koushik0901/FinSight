import { useEffect, useId, useRef } from "react";
import FocusLock from "react-focus-lock";
import { createPortal } from "react-dom";
import type { ReactNode } from "react";

interface DrawerProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  width?: number;
  /** Stack above an already-open dialog (e.g. the CSV import dialog opening the
   *  Add-account drawer inline). Without this the drawer renders behind the
   *  dialog's backdrop. */
  elevated?: boolean;
}

export default function Drawer({ open, onClose, title, children, width = 480, elevated = false }: DrawerProps) {
  const titleId = useId();
  const lastActive = useRef<HTMLElement | null>(null);

  // Restore focus on close.
  useEffect(() => {
    if (open) {
      lastActive.current = (document.activeElement as HTMLElement) ?? null;
    } else if (lastActive.current) {
      lastActive.current.focus();
      lastActive.current = null;
    }
  }, [open]);

  // ESC key closes the drawer.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return createPortal(
    <FocusLock returnFocus={false}>
      <div className={elevated ? "drawer-root drawer-root-elevated" : "drawer-root"}>
        <div
          className="drawer-backdrop"
          data-testid="drawer-backdrop"
          onClick={onClose}
        />
        <div
          className="drawer-panel"
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
          style={{ width }}
        >
          <header className="drawer-header">
            <h2 id={titleId}>{title}</h2>
            <button type="button" aria-label="Close" onClick={onClose}>×</button>
          </header>
          <div className="drawer-body">{children}</div>
        </div>
      </div>
    </FocusLock>,
    document.body
  );
}
