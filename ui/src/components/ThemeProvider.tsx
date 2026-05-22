import { useEffect, type ReactNode } from "react";
import { ACCENTS, useTweaks } from "../state/tweaks";

export function ThemeProvider({ children }: { children: ReactNode }) {
  const { theme, density, accent, privacy } = useTweaks();

  useEffect(() => {
    const root = document.documentElement;
    root.setAttribute("data-theme", theme);
    root.setAttribute("data-density", density);
    root.setAttribute("data-privacy", privacy ? "on" : "off");

    const { hex, ink } = ACCENTS[accent];
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    root.style.setProperty("--accent", hex);
    root.style.setProperty("--accent-ink", ink);
    root.style.setProperty("--accent-2", `rgba(${r}, ${g}, ${b}, 0.14)`);
    root.style.setProperty("--accent-3", `rgba(${r}, ${g}, ${b}, 0.28)`);
    root.style.setProperty("--accent-glow", `0 0 60px rgba(${r}, ${g}, ${b}, 0.20)`);
  }, [theme, density, accent, privacy]);

  return <>{children}</>;
}
