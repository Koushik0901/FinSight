import { create } from "zustand";
import type { CsvImportMapping } from "../api/bindings";

export type OnboardingStep = "welcome" | "connect" | "categories" | "agent";

interface OnboardingStore {
  step: OnboardingStep;
  reachedSteps: Set<OnboardingStep>;
  mappingDraft: Partial<CsvImportMapping> | null;
  setStep: (s: OnboardingStep) => void;
  markReached: (s: OnboardingStep) => void;
  setMappingDraft: (m: Partial<CsvImportMapping> | null) => void;
  reset: () => void;
}

const ORDER: OnboardingStep[] = ["welcome", "connect", "categories", "agent"];

export const useOnboardingStore = create<OnboardingStore>((set) => ({
  step: "welcome",
  reachedSteps: new Set(["welcome"]),
  mappingDraft: null,
  setStep: (step) => set((s) => ({
    step,
    reachedSteps: new Set([...s.reachedSteps, step]),
  })),
  markReached: (step) => set((s) => ({
    reachedSteps: new Set([...s.reachedSteps, step]),
  })),
  setMappingDraft: (mappingDraft) => set({ mappingDraft }),
  reset: () => set({ step: "welcome", reachedSteps: new Set(["welcome"]), mappingDraft: null }),
}));

export const STEP_ORDER = ORDER;
