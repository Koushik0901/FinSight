import { useEffect, useRef } from "react";
import Copilot from "../Copilot";

/**
 * Mobile Copilot — reuses the desktop Copilot runtime but adapts the chrome
 * for thumb use: hides the 240px thread sidebar, makes the header/composer
 * sticky above the bottom nav, and offsets for the on-screen keyboard via
 * visualViewport.
 *
 * Implementation: render the existing Copilot (which already ships the full
 * Thread / TauriRuntime / generative UI) inside a mobile-specific wrapper.
 * CSS overrides in mobile.css flip .copilot-screen from absolute → relative
 * and hide the sidebar when inside .mobile-shell.
 */
export default function MobileCopilot() {
  const wrapRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const vv = window.visualViewport;
    if (!vv) return;
    const onResize = () => {
      // Keyboard up → vv.height < window.innerHeight. Use inset to pad composer.
      const inset = Math.max(0, window.innerHeight - vv.height - vv.offsetTop);
      if (wrapRef.current) wrapRef.current.style.setProperty("--keyboard-inset", `${inset}px`);
    };
    vv.addEventListener("resize", onResize);
    vv.addEventListener("scroll", onResize);
    onResize();
    return () => {
      vv.removeEventListener("resize", onResize);
      vv.removeEventListener("scroll", onResize);
    };
  }, []);

  return (
    <div
      ref={wrapRef}
      className="mobile-copilot"
      style={
        {
          // Fallback when visualViewport not available
          ["--keyboard-inset" as string]: "0px",
          display: "flex",
          flexDirection: "column",
          height: "calc(100dvh - 56px - 72px - env(safe-area-inset-top) - env(safe-area-inset-bottom))",
          minHeight: "calc(100dvh - 56px - 72px)",
          overflow: "hidden",
        } as React.CSSProperties
      }
    >
      {/* The existing Copilot expects to own .copilot-screen. Inside mobile we override its absolute layout */}
      <Copilot />
      <style>{`
        .mobile-copilot .copilot-screen {
          position: relative !important;
          inset: auto !important;
          height: 100% !important;
          min-height: 0 !important;
          display: flex !important;
          flex-direction: column !important;
          grid-template-columns: 1fr !important;
          background: var(--bg) !important;
        }
        .mobile-copilot .copilot-sidebar {
          display: none !important;
        }
        .mobile-copilot .copilot-main,
        .mobile-copilot .copilot-thread-wrap,
        .mobile-copilot .copilot-thread {
          min-height: 0 !important;
          flex: 1 !important;
        }
        .mobile-copilot .copilot-composer-wrap {
          position: sticky !important;
          bottom: calc(var(--keyboard-inset, 0px)) !important;
          background: linear-gradient(to top, var(--bg) 90%, transparent) !important;
          padding-bottom: calc(10px + env(safe-area-inset-bottom) + var(--keyboard-inset, 0px)) !important;
          z-index: 5;
        }
        .mobile-copilot .copilot-viewport {
          padding-bottom: 16px !important;
        }
        /* Prompt chips: horizontal scroll on phone */
        .mobile-copilot .copilot-prompts-grid,
        .mobile-copilot .copilot-finsight-chat .copilot-prompts-grid {
          flex-wrap: nowrap !important;
          overflow-x: auto !important;
          scrollbar-width: none !important;
          justify-content: flex-start !important;
        }
        .mobile-copilot .copilot-prompts-grid::-webkit-scrollbar { display: none !important; }
      `}</style>
    </div>
  );
}
