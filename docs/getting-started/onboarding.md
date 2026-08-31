# Onboarding

Onboarding is account-first. You land on it after creating the admin account; you can also re-run it from **Settings** via “Reset onboarding”.

## Steps

1. **Choose how to start.** Create a manual account (checking, savings, credit, investment, cash, loan, or other) or connect SimpleFIN.
2. **SimpleFIN (optional).** Paste the access URL you obtained from your bank via SimpleFIN Bridge. FinSight exchanges it only when you explicitly connect and when you synchronize. The URL is stored inside your encrypted database, not a global secret store.
3. **Import CSV history (optional).** Upload a CSV in the import drawer. FinSight parses and stages candidates, deduplicates against existing transactions, and shows a review queue before finalizing. You can import years of history here.
4. **Categories.** Keep the seeded set or tailor it. Every category carries a `spending_type` (Need/Want/Saving/Investment) that powers the Conscious Spending view on Budget and the Copilot’s allocation checks.
5. **Provider (optional).** Configure an AI provider if you want auto-categorization and the Copilot. Skip it to run fully offline; you can add it later at **Settings → Agent**.

Progress is persisted — you can close the tab and return. The “Reset onboarding” control in Settings clears the onboarding-seen flag for testing and returns you to the wizard flow.

## Tips

- For manual accounts, balance history is derived from transactions; you do not enter a separate balance ledger.
- SimpleFIN accounts show a sync status and last-run timestamp. Use **Accounts → Synchronize** to pull new activity.
- If you imported CSVs and see unconverted currencies, check [Currencies](/guide/accounts) — amounts without a known conversion are kept separate, never silently summed.

Next: [Importing Your Data](/getting-started/importing-data).
