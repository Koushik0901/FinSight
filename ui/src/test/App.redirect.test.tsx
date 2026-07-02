import { describe, it, expect, vi } from "vitest";
import { render, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { shouldShowOnboarding, useOnboardingRedirect } from "../hooks/useOnboardingRedirect";
import type { OnboardingState } from "../api/client";

function LocationReader() {
  const location = useLocation();
  return <span data-testid="location">{location.pathname}</span>;
}

function TestRedirect({ onboarding }: { onboarding: OnboardingState | undefined }) {
  useOnboardingRedirect(onboarding);
  return <LocationReader />;
}

function renderRedirect(initialPath: string, onboarding: OnboardingState | undefined) {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <TestRedirect onboarding={onboarding} />
    </MemoryRouter>
  );
}

describe("useOnboardingRedirect", () => {
  it("redirects empty DB to /onboarding", async () => {
    const { getByTestId } = renderRedirect("/", {
      account_count: 0,
      category_count: 0,
      completion_marked: false,
    });
    await waitFor(() => {
      expect(getByTestId("location").textContent).toBe("/onboarding");
    });
  });

  it("does not redirect when accounts exist", async () => {
    const { getByTestId } = renderRedirect("/", {
      account_count: 3,
      category_count: 5,
      completion_marked: false,
    });
    await waitFor(() => {
      expect(getByTestId("location").textContent).toBe("/");
    });
  });

  it("does not redirect when completion_marked even if accounts empty", async () => {
    const { getByTestId } = renderRedirect("/", {
      account_count: 0,
      category_count: 0,
      completion_marked: true,
    });
    await waitFor(() => {
      expect(getByTestId("location").textContent).toBe("/");
    });
  });

  it("stays on /onboarding when already there", async () => {
    const { getByTestId } = renderRedirect("/onboarding", {
      account_count: 0,
      category_count: 0,
      completion_marked: false,
    });
    await waitFor(() => {
      expect(getByTestId("location").textContent).toBe("/onboarding");
    });
  });
});

describe("shouldShowOnboarding", () => {
  it("returns true only when no accounts and not completed", () => {
    expect(
      shouldShowOnboarding({ account_count: 0, category_count: 0, completion_marked: false })
    ).toBe(true);
    expect(
      shouldShowOnboarding({ account_count: 3, category_count: 5, completion_marked: false })
    ).toBe(false);
    expect(
      shouldShowOnboarding({ account_count: 0, category_count: 0, completion_marked: true })
    ).toBe(false);
    expect(shouldShowOnboarding(undefined)).toBe(false);
  });
});
