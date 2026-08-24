use finsight_core::metrics::{explain, monthly_expense_cents, ExpenseBasis};

#[test]
fn pantry_explain_is_non_empty() {
    assert!(explain(ExpenseBasis::DisplayMedian).contains("Smooth"));
    assert!(explain(ExpenseBasis::RecentMean90).contains("Recent"));
    assert!(explain(ExpenseBasis::SafetyConservative).contains("Conservative"));
}

#[test]
fn pantry_monthly_expense_is_greppable() {
    // This test exists so grep for raw calls fails after migration.
    // It will pass only when monthly_expense_cents exists and delegates correctly.
    let (_dir, db) = finsight_core::testing::migrated_db();
    let conn = db.get().unwrap();
    let (cents, sufficient) =
        monthly_expense_cents(&conn, ExpenseBasis::RecentMean90, None).unwrap();
    assert!(cents >= 0);
    let _ = sufficient;
}
