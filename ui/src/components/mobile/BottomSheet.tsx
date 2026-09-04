import { useEffect, useId, useRef, useCallback } from "react";
import FocusLock from "react-focus-lock";
import { createPortal } from "react-dom";
interface BottomSheetProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
  /** Optional description for aria-describedby */
  description?: string;
  /** Full-height drill-down (100dvh, no radius) for dense detail screens */
  fullHeight?: boolean;
  /** Hide the drag-handle (use for confirmation sheets where it would confuse) */
  hideHandle?: boolean;
}

/**
 * Phone-native bottom sheet — replaces Desktop `Drawer` (480px side panel) on mobile.
 * - Slides from bottom with handle, backdrop blur, safe-area padding.
 * - Focus: restores to trigger on close, traps via ESC/backdrop/drag.
 * - Body scroll locked while open.
 * - Same token palette as Drawer — no hardcoded hex.
 */
export function BottomSheet({
  open,
  onClose,
  title,
  children,
  description,
  fullHeight = false,
  hideHandle = false,
}: BottomSheetProps) {
  const titleId = useId();
  const descId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const lastActive = useRef<HTMLElement | null>(null);
  const startY = useRef<number | null>(null);
  const dragging = useRef(false);

  // Focus restore
  useEffect(() => {
    if (open) {
      lastActive.current = (document.activeElement as HTMLElement) ?? null;
      // Focus the panel for a11y, but not before animation starts — defer one frame.
      requestAnimationFrame(() => panelRef.current?.focus());
    } else if (lastActive.current) {
      lastActive.current.focus();
      lastActive.current = null;
    }
  }, [open]);

  // ESC closes
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

  // Body scroll lock (iOS safe — position fixed would jump).
  useEffect(() => {
    if (!open) return;
    const prev = document.body.style.overflow;
    const prevPad = document.body.style.paddingRight;
    const scrollbarW = window.innerWidth - document.documentElement.clientWidth;
    document.body.style.overflow = "hidden";
    if (scrollbarW > 0) document.body.style.paddingRight = `${scrollbarW}px`;
    // inert the main shell so background is not reachable via Tab or screen reader
    const shell = document.querySelector(".mobile-shell") as HTMLElement | null;
    const prevInert = shell?.hasAttribute("inert");
    if (shell) shell.setAttribute("inert", "");
    return () => {
      document.body.style.overflow = prev;
      document.body.style.paddingRight = prevPad;
      if (shell && !prevInert) shell.removeAttribute("inert");
    };
  }, [open]);

  // Drag to dismiss — track start, if drag > 96px downward dismiss
  const onTouchStart = useCallback((e: React.TouchEvent) => {
    if (fullHeight) return;
    startY.current = e.touches[0]?.clientY ?? null;
    dragging.current = false;
  }, [fullHeight]);

  const onTouchMove = useCallback((e: React.TouchEvent) => {
    if (fullHeight || startY.current == null) return;
    const dy = (e.touches[0]?.clientY ?? 0) - startY.current;
    if (dy > 12) dragging.current = true;
    // Follow finger visually (capped)
    if (dragging.current && panelRef.current) {
      const clamped = Math.max(0, Math.min(dy, 320));
      panelRef.current.style.transform = `translateY(${clamped}px)`;
    }
    // Prevent body overscroll while dragging sheet
    if (dragging.current) e.preventDefault();
  }, [fullHeight]);

  const onTouchEnd = useCallback((e: React.TouchEvent) => {
    if (fullHeight || startY.current == null) return;
    const endY = e.changedTouches[0]?.clientY ?? startY.current;
    const dy = endY - startY.current;
    if (panelRef.current) panelRef.current.style.transform = "";
    if (dy > 96) {
      onClose();
    }
    startY.current = null;
    dragging.current = false;
  }, [fullHeight, onClose]);

  if (!open) return null;

  const content = (
    <FocusLock returnFocus={false} disabled={!open}>
      <div
        className={`mobile-sheet-root${open ? " open" : ""}`}
        aria-hidden={open ? undefined : true}
      >
        <div
          className="mobile-sheet-backdrop"
          onClick={onClose}
          data-testid="bottom-sheet-backdrop"
        />
        <div
          ref={panelRef}
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
          aria-describedby={description ? descId : undefined}
          tabIndex={-1}
          className={`mobile-sheet-panel${fullHeight ? " full" : ""}`}
          onTouchStart={onTouchStart}
          onTouchMove={onTouchMove}
          onTouchEnd={onTouchEnd}
          style={{ outline: "none" }}
        >
          {!hideHandle && !fullHeight && <div className="mobile-sheet-handle" aria-hidden="true" />}
          <div className="mobile-sheet-header">
            <h2 id={titleId}>{title}</h2>
            <button
              type="button"
              className="mobile-sheet-close"
              aria-label="Close"
              onClick={onClose}
            >
              ×
            </button>
          </div>
          {description ? (
            <p id={descId} className="sr-only">
              {description}
            </p>
          ) : null}
          <div className="mobile-sheet-body">{children}</div>
        </div>
      </div>
    </FocusLock>
  );

  return createPortal(content, document.body);
}

export default BottomSheet;
