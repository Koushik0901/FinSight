import { create } from "zustand";
import { persist } from "zustand/middleware";

export type Theme = "dark" | "light";
export type Density = "cozy" | "compact";
export type AccentId = "lime" | "emerald" | "sky" | "violet" | "amber" | "rose";

export const ACCENTS: Record<AccentId, { hex: string; ink: string }> = {
  lime: { hex: "#C9F950", ink: "#0A0F02" },
  emerald: { hex: "#34D399", ink: "#04130C" },
  sky: { hex: "#60A5FA", ink: "#02101F" },
  violet: { hex: "#A78BFA", ink: "#0F0820" },
  amber: { hex: "#FBBF24", ink: "#1A1300" },
  rose: { hex: "#FB7185", ink: "#1F0710" },
};

interface State {
  theme: Theme;
  density: Density;
  accent: AccentId;
  privacy: boolean;
  setTheme: (t: Theme) => void;
  setDensity: (d: Density) => void;
  setAccent: (a: AccentId) => void;
  setPrivacy: (p: boolean) => void;
}

export const useTweaks = create<State>()(
  persist(
    (set) => ({
      theme: "dark",
      density: "cozy",
      accent: "lime",
      privacy: false,
      setTheme: (theme) => set({ theme }),
      setDensity: (density) => set({ density }),
      setAccent: (accent) => set({ accent }),
      setPrivacy: (privacy) => set({ privacy }),
    }),
    { name: "finsight.tweaks" }
  )
);
