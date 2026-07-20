import { create } from "zustand";
import { persist } from "zustand/middleware";

/**
 * Tracks which Copilot clarifications are still waiting on the user.
 *
 * The composer is replaced by the clarification while one is outstanding, so
 * "is anything pending" has to be readable from outside the message that
 * rendered it. A store is the smallest thing that spans both — no new
 * conversation state, no migration, and the question itself still lives in the
 * persisted message blocks.
 *
 * `resolved` is persisted because the clarification block stays in the thread
 * forever. Without it, answering a question and then reloading would re-block
 * the composer on a question the user already dealt with.
 */
export type PendingClarification = {
  id: string;
  question: string;
};

type ClarificationState = {
  /** Ids the user has answered or dismissed. */
  resolved: Record<string, true>;
  /** The clarification currently blocking the composer, if any. */
  pending: PendingClarification | null;
  /** Called by a rendered clarification that has not been dealt with yet. */
  requestBlock: (clarification: PendingClarification) => void;
  /** Answered or dismissed — either way it stops blocking, permanently. */
  release: (id: string) => void;
  /**
   * Stop blocking without marking the question dealt with.
   *
   * Called when the clarification stops being on screen for a reason that is
   * not an answer — switching conversation, starting a new thread. Without it
   * the composer stays blocked on a question the user can no longer see or
   * dismiss, which is the exact trap the dismiss button exists to prevent.
   * Deliberately does NOT add to `resolved`: the question is still unanswered,
   * so revisiting that thread should block again.
   */
  clearIfPending: (id: string) => void;
};

export const useClarifications = create<ClarificationState>()(
  persist(
    (set, get) => ({
      resolved: {},
      pending: null,
      requestBlock: (clarification) => {
        const { resolved, pending } = get();
        // Already dealt with, or already the one blocking — nothing to do.
        // Bailing out here keeps this safe to call from a render effect.
        if (resolved[clarification.id]) return;
        if (pending?.id === clarification.id) return;
        set({ pending: clarification });
      },
      release: (id) =>
        set((state) => ({
          resolved: { ...state.resolved, [id]: true },
          pending: state.pending?.id === id ? null : state.pending,
        })),
      clearIfPending: (id) =>
        set((state) => (state.pending?.id === id ? { pending: null } : state)),
    }),
    {
      name: "finsight.clarifications",
      // `pending` is deliberately not persisted: it is re-established by the
      // block rendering, which is the authoritative source. Persisting it
      // could block the composer with no visible question to answer.
      partialize: (state) => ({ resolved: state.resolved }),
    },
  ),
);
