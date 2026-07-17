# Account-first onboarding redesign

Date: 2026-07-16
Status: Approved direction; awaiting written-spec review

## Purpose

Rework onboarding so it follows the user's financial setup rather than presenting account creation and transaction import as equivalent actions. The new flow must also fix the broken global skip action, make the welcome name field readable and consistent with the rest of the app, and remove personal names from user-facing examples and related fixtures.

## Goals

- Make "Skip setup" leave onboarding reliably.
- Use a visible, styled, optional name field with the placeholder `e.g. John Doe`.
- Replace the account-owner placeholder with a generic example such as `Add a person (e.g. Jane Doe)`.
- Separate creating/discovering accounts from adding transaction history.
- Preserve SimpleFIN's useful ability to discover and create connected accounts automatically.
- Let users defer any individual setup stage without trapping them.

## Non-goals

- Changing the account, CSV-import, or SimpleFIN backend contracts.
- Building account matching between manually created accounts and SimpleFIN accounts.
- Reworking categories or AI-provider setup beyond renumbering them in the longer flow.
- Replacing the existing `AccountDrawer`, `ImportMappingDialog`, or `SimpleFinDialog` when they already provide the required workflow.

## Chosen flow

The onboarding wizard becomes five steps:

1. **Welcome** - explain the product and optionally identify the current user.
2. **Accounts** - create manual account shells or connect SimpleFIN to discover accounts.
3. **History** - import CSV history into a chosen manual account and show SimpleFIN-linked accounts as already connected.
4. **Categories** - confirm starter categories.
5. **AI setup** - configure a provider or defer configuration.

This is an account-first hybrid. It keeps account creation and transaction history distinct while acknowledging that SimpleFIN supplies both account identity and synchronized activity. A strict account-first design that forced users to create account shells before SimpleFIN was rejected because it would add duplicate work and require a new matching system. A source-first design was rejected because it would preserve the current confusing mental model.

## Detailed interaction design

### 1. Welcome

The name field uses the shared `Input` component so it receives the same surface, border, text color, placeholder color, focus state, and accessibility behavior as other forms. Its label remains explicit that the value is optional and explains that it improves internal-transfer recognition. The placeholder is `e.g. John Doe`; no real person's name appears in the rendered example.

"Get started" keeps the current best-effort behavior: if a name is provided, create the household member and mark it as the current user, but do not block onboarding if that optional operation fails.

"Skip setup" is a completion action, not navigation alone. It must:

1. call `markOnboardingComplete`;
2. wait for success;
3. navigate to Today with history replacement so Back does not reopen onboarding.

While the command is pending, the skip button is disabled and shows progress. If the command fails, onboarding stays visible and an inline error explains that setup could not be skipped. This prevents the existing redirect loop caused by navigating away while `completion_marked` remains false.

### 2. Accounts

The old `StepConnect` is split. The Accounts step owns only account establishment:

- an empty state explaining that accounts are the containers for balances and history;
- a clear "Add account" action that opens the existing `AccountDrawer`;
- a secondary "Connect SimpleFIN" action that opens the existing `SimpleFinDialog` and lets SimpleFIN discover accounts automatically;
- a live roster of accounts returned by `useAccounts`, including bank/name, type, and whether the account is manual or SimpleFIN-linked;
- a concise count and readiness state.

The page does not offer CSV import or manual transaction entry. Users with accounts continue to History. Users without accounts may choose "I'll do this later" and continue without being trapped.

### 3. History

The History step answers one question: how should activity reach each existing account?

- Manual accounts show an "Import CSV" action. Picking a file opens the existing `ImportMappingDialog` with `defaultAccountId`, so the chosen account is already selected while remaining changeable.
- SimpleFIN-linked accounts show a connected/synchronized state rather than a redundant CSV action. The initial SimpleFIN workflow may already have synchronized activity; this screen communicates that outcome instead of asking the user to reconnect.
- A general "Connect SimpleFIN" action remains available when no SimpleFIN-linked account exists, because connecting may discover accounts the user did not create manually.
- The screen shows account and transaction totals so users can see whether history arrived.
- Users can continue with no transactions. "Continue" and "Do this later" both lead to Categories; copy makes clear that CSV imports and sync remain available after onboarding.

Manual transaction entry is removed from onboarding. It remains available in the main app, where it is a normal ledger action rather than a setup method.

### 4. Categories and 5. AI setup

Existing behavior remains intact. Step numbers, progress labels, and state ordering update to reflect the two new account/history stages. Completing or deferring AI setup continues to mark onboarding complete before navigating to Today.

## Component boundaries

- `Onboarding.tsx` renders the five-step sequence and owns route transitions.
- `state/onboarding.ts` defines the new `accounts` and `history` step identifiers and reached-step ordering.
- `StepWelcome.tsx` owns optional-name capture and reliable global skip behavior.
- `StepAccounts.tsx` owns the account roster plus account creation and SimpleFIN discovery drawers.
- `StepHistory.tsx` owns CSV file selection, account-scoped import dialogs, transaction/account status, and the optional SimpleFIN entry point.
- Existing account, import, and SimpleFIN components remain reusable units; they are not forked into onboarding-only implementations.

## Error and loading behavior

- Account and transaction queries display a compact loading state and a recoverable inline error.
- Browser preview copy continues to explain that writes require the desktop runtime.
- A failed global skip never navigates away.
- Drawer/dialog failures continue to use their existing error handling and toasts.
- Step navigation never depends on a transaction count, and an empty account list never blocks the explicit defer action.

## Privacy cleanup

User-facing examples use only generic names. The two known personal placeholders are replaced, and related account/onboarding test fixtures that copied those names are renamed to generic people. A repository search verifies that those personal names no longer appear in onboarding or account UI sources and tests. Unrelated domain fixtures are changed only when they expose the same copied personal data; behavior-specific parsing fixtures are not broadened into an unrelated refactor.

## Accessibility and responsive behavior

- The welcome field has a programmatic label and standard focus styling.
- Progress controls retain `aria-current`, descriptive labels, and reached-step keyboard navigation.
- Account/history actions use real buttons with explicit accessible names.
- Status counts use `aria-live` only where updates need announcement.
- The existing single-column onboarding breakpoint remains, with account rows and action groups wrapping without horizontal overflow.

## Verification

Focused frontend tests must prove:

- global skip calls `markOnboardingComplete` before navigation;
- failed skip stays on onboarding and exposes an error;
- the name input renders through shared field styling and uses `John Doe`;
- the account owner placeholder is generic;
- Get Started still saves a supplied name best-effort and advances;
- Accounts contains account creation and SimpleFIN discovery but no CSV/manual-transaction actions;
- History presents account-scoped CSV import and source-appropriate SimpleFIN state;
- users can defer Accounts and History;
- progress ordering and reached-step navigation work across all five steps.

After focused tests, run the full frontend test suite and TypeScript type-check. Perform a rendered onboarding walkthrough in the real desktop runtime when available, specifically checking input contrast, skip navigation, drawer layering, both setup paths, and responsive layout.

## Acceptance criteria

The work is complete when every numbered user issue is demonstrably resolved: skip persists completion and reaches Today, both PII examples are removed, the name field is visibly styled and readable, account establishment and history import are separate screens, SimpleFIN discovery remains efficient, and automated plus rendered verification cover the revised flow.
