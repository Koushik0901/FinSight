import { create } from "zustand";
import type { CsvImportMapping } from "../api/openapiClient";

export type OnboardingStep = "accounts" | "history" | "categories" | "agent";

interface OnboardingStore {
  step: OnboardingStep;
  reachedSteps: Set<OnboardingStep>;
  mappingDraft: Partial<CsvImportMapping> | null;
  setStep: (s: OnboardingStep) => void;
  markReached: (s: OnboardingStep) => void;
  setMappingDraft: (m: Partial<CsvImportMapping> | null) => void;
  reset: () => void;
}

const ORDER: OnboardingStep[] = ["accounts", "history", "categories", "agent"];

export const useOnboardingStore = create<OnboardingStore>((set) => ({
  step: "accounts",
  reachedSteps: new Set(["accounts"]),
  mappingDraft: null,
  setStep: (step) => set((s) => ({
    step,
    reachedSteps: new Set([...s.reachedSteps, step]),
  })),
  markReached: (step) => set((s) => ({
    reachedSteps: new Set([...s.reachedSteps, step]),
  })),
  setMappingDraft: (mappingDraft) => set({ mappingDraft }),
  reset: () => set({ step: "accounts", reachedSteps: new Set(["accounts"]), mappingDraft: null }),
}));

export const STEP_ORDER = ORDER;
