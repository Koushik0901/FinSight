import { useEffect, useId, useRef, useState } from "react";
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
  const [render, setRender] = useState(open);
  const [closing, setClosing] = useState(false);
  const timerRef = useRef<number | null>(null);

  // Mount immediately when opening; delay unmount to play exit animation.
  useEffect(() => {
    if (open) {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
        timerRef.current = null;
      }
      setRender(true);
      setClosing(false);
      return;
    }
    if (render && !closing) {
      setClosing(true);
      const isMobile = typeof window !== "undefined" && window.matchMedia("(max-width: 640px)").matches;
      const dur = isMobile ? 200 : 180;
      timerRef.current = window.setTimeout(() => {
        setRender(false);
        setClosing(false);
        timerRef.current = null;
      }, dur);
    }
  }, [open, render, closing]);

  // Cleanup timer on unmount.
  useEffect(() => {
    return () => {
      if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    };
  }, []);

  // Restore focus on close (after exit animation completes).
  useEffect(() => {
    if (open) {
      lastActive.current = (document.activeElement as HTMLElement) ?? null;
    } else if (!render && lastActive.current) {
      lastActive.current.focus();
      lastActive.current = null;
    }
  }, [open, render]);

  // ESC key closes the drawer. Kept active while rendered so closing animation can be triggered via keyboard.
  useEffect(() => {
    if (!render || closing) return;
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, render, closing, onClose]);

  if (!render) return null;

  const rootClass = [
    "drawer-root",
    elevated ? "drawer-root-elevated" : "",
    closing ? "is-exiting" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return createPortal(
    <FocusLock returnFocus={false}>
      <div className={rootClass}>
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
