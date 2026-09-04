import { useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { useAccounts } from "../../api/hooks/accounts";
import { useCategoriesWithSpending } from "../../api/hooks/transactions";
import { useInfiniteTransactions } from "../../api/hooks/transactions";
import type { Transaction, TxnFilterInput } from "../../api/openapiClient";
import { money } from "../../utils/format";
import { useDebouncedValue } from "../../utils/useDebouncedValue";
import * as I from "../../components/Icons";
import { MobileList, MobileListItem } from "../../components/mobile/MobileList";
import { BottomSheet } from "../../components/mobile/BottomSheet";
import { SegmentedControl } from "../../components/mobile/SegmentedControl";
import { MobileEmptyState } from "../../components/mobile/MobileEmptyState";
import TransactionDrawer from "../../components/TransactionDrawer";

const PRESETS = [
  { value: "all", label: "All" },
  { value: "needs_review", label: "Needs review" },
  { value: "anomalies", label: "Unusual" },
  { value: "no_category", label: "No category" },
  { value: "transfer_review", label: "Transfers" },
] as const;

type Preset = (typeof PRESETS)[number]["value"];

function relativeDate(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  const diffDays = Math.floor((now.getTime() - d.getTime()) / 86400000);
  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  if (diffDays < 7) return `${diffDays}d ago`;
  if (diffDays < 30) return `${Math.floor(diffDays / 7)}w ago`;
  return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

function formatAmount(cents: number, currency: string): string {
  const sign = cents < 0 ? "-" : "";
  return `${sign}${money(Math.abs(cents), { currency })}`;
}

function spendingTypeLabel(catId: string | null, cats: Array<{ id: string; spendingType?: string; label: string }>): string | null {
  if (!catId) return null;
  const c = cats.find((x) => x.id === catId) as unknown as { spendingType?: string } | undefined;
  return (c as { spendingType?: string } | undefined)?.spendingType ?? null;
}

export default function MobileTransactions() {
  const { data: accounts = [] } = useAccounts();
  const { data: categories = [] } = useCategoriesWithSpending();
  const [searchParams, setSearchParams] = useSearchParams();
  const [search, setSearch] = useState(searchParams.get("q") ?? "");
  const debouncedSearch = useDebouncedValue(search, 300);
  const [filterOpen, setFilterOpen] = useState(false);
  const [preset, setPreset] = useState<Preset>((searchParams.get("filter") as Preset) ?? "all");
  const [accountFilter, setAccountFilter] = useState<string | null>(searchParams.get("account") ?? null);
  const [dateRange, setDateRange] = useState<string>(searchParams.get("range") ?? "all");
  const [detailTxn, setDetailTxn] = useState<Transaction | null>(null);
  const [editOpen, setEditOpen] = useState(false);

  const accountById = useMemo(() => Object.fromEntries(accounts.map((a) => [a.id, a])), [accounts]);
  const primaryCurrency = accounts[0]?.currency ?? "USD";

  const filterValue: Omit<TxnFilterInput, "limit" | "offset"> = useMemo(() => {
    let startDate: string | null = null;
    const endDate: string | null = null;
    if (dateRange !== "all") {
      const now = new Date();
      if (dateRange === "week") {
        const d = new Date(now); d.setDate(d.getDate() - 7); startDate = d.toISOString().slice(0, 10);
      } else if (dateRange === "month") {
        startDate = new Date(now.getFullYear(), now.getMonth(), 1).toISOString().slice(0, 10);
      } else if (dateRange === "30d") {
        const d = new Date(now); d.setDate(d.getDate() - 30); startDate = d.toISOString().slice(0, 10);
      }
    }
    return {
      accountId: accountFilter,
      search: debouncedSearch || null,
      filterPreset: preset === "all" ? null : preset,
      startDate,
      endDate,
    };
  }, [accountFilter, debouncedSearch, preset, dateRange]);
  const {
    data: pages,
    isLoading,
    error,
    fetchNextPage,
    hasNextPage,
    isFetchingNextPage,
  } = useInfiniteTransactions(filterValue);

  const transactions = useMemo(() => pages?.pages.flat() ?? [], [pages]);

  const handlePresetChange = (next: Preset) => {
    setPreset(next);
    setSearchParams((prev) => {
      const p = new URLSearchParams(prev);
      if (next === "all") p.delete("filter");
      else p.set("filter", next);
      return p;
    }, { replace: true });
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12, padding: 16, paddingBottom: 24 }}>
      {/* Search — prominent, thumb-friendly */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 10,
          minHeight: 44,
          padding: "0 12px",
          background: "var(--surface)",
          border: "1px solid var(--line)",
          borderRadius: 12,
        }}
      >
        <I.Search width={16} height={16} style={{ color: "var(--ink-faint)", flexShrink: 0 }} aria-hidden="true" />
        <input
          value={search}
          onChange={(e) => {
            setSearch(e.target.value);
            const v = e.target.value;
            setSearchParams((prev) => {
              const p = new URLSearchParams(prev);
              if (v) p.set("q", v);
              else p.delete("q");
              return p;
            }, { replace: true });
          }}
          placeholder="Search merchants, notes…"
          aria-label="Search transactions"
          style={{
            flex: 1,
            minWidth: 0,
            border: 0,
            background: "transparent",
            outline: "none",
            fontSize: 16,
            color: "var(--ink)",
          }}
        />
        {search ? (
          <button type="button" aria-label="Clear search" onClick={() => setSearch("")} style={{ border: 0, background: "transparent", color: "var(--ink-faint)", padding: 6 }}>
            <I.X width={14} height={14} />
          </button>
        ) : null}
      </div>

      {/* Filter chips — horizontal scroll, opens sheet */}
      <div style={{ display: "flex", gap: 8, alignItems: "center", overflowX: "auto", scrollbarWidth: "none", paddingBottom: 2 }}>
        <button
          type="button"
          onClick={() => setFilterOpen(true)}
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 6,
            minHeight: 36,
            padding: "0 12px",
            borderRadius: 999,
            border: "1px solid var(--line)",
            background: preset !== "all" || accountFilter || dateRange !== "all" ? "var(--accent)" : "var(--surface)",
            color: preset !== "all" || accountFilter || dateRange !== "all" ? "var(--accent-ink)" : "var(--ink)",
            fontWeight: 600,
            fontSize: 13,
            whiteSpace: "nowrap",
          }}
        >
          <I.Filter width={14} height={14} /> Filters
          {(preset !== "all" || accountFilter || dateRange !== "all") ? <span style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--accent-ink)", display: "inline-block" }} /> : null}
        </button>
        <div style={{ display: "inline-flex", gap: 6, flexWrap: "nowrap" }}>
          {PRESETS.map((p) => (
            <button
              key={p.value}
              type="button"
              onClick={() => handlePresetChange(p.value as Preset)}
              aria-pressed={preset === p.value}
              style={{
                minHeight: 36,
                padding: "0 12px",
                borderRadius: 999,
                border: "1px solid var(--line)",
                background: preset === p.value ? "var(--surface-2)" : "transparent",
                color: preset === p.value ? "var(--ink)" : "var(--ink-mute)",
                fontWeight: preset === p.value ? 650 : 500,
                fontSize: 13,
                whiteSpace: "nowrap",
              }}
            >
              {p.label}
            </button>
          ))}
        </div>
      </div>

      {/* List */}
      {isLoading ? (
        <div className="stub"><span className="spinner" aria-hidden="true" /> Loading…</div>
      ) : error ? (
        <div style={{ padding: 16, border: "1px solid var(--line)", borderRadius: 16, background: "var(--surface)", color: "var(--negative)" }}>Failed to load transactions.</div>
      ) : transactions.length === 0 ? (
        <MobileEmptyState
          icon={<I.Search width={24} height={24} />}
          title="No transactions"
          description={search || preset !== "all" ? "Try adjusting your search or filters." : "Import a statement to see transactions here."}
          primaryAction={
            search || preset !== "all" ? (
              <button className="btn" type="button" onClick={() => { setSearch(""); handlePresetChange("all"); setAccountFilter(null); setDateRange("all"); }}>
                Clear filters
              </button>
            ) : undefined
          }
        />
      ) : (
        <>
          <div style={{ fontSize: 12, color: "var(--ink-faint)", padding: "0 4px" }}>
            {transactions.length} {transactions.length === 1 ? "transaction" : "transactions"}
            {hasNextPage ? " · scroll to load more" : ""}
          </div>
          <MobileList ariaLabel="Transactions">
            {transactions.map((t) => {
              const catLabel = (t as unknown as { category_label?: string | null }).category_label ?? t.category_label ?? null;
              const catColor = t.category_color ?? (catLabel ? "var(--ink-faint)" : undefined);
              const acct = accountById[t.account_id];
              const spendType = spendingTypeLabel(t.category_id ?? null, categories as unknown as Array<{ id: string; spendingType?: string; label: string }>);
              const subtitle = `${catLabel ?? "Uncategorized"}${spendType ? ` · ${spendType}` : ""} · ${relativeDate(t.posted_at)}${acct ? ` · ${acct.name}` : ""}`;
              // Keep desktop snake_case + mobile camelCase both covered
              const isAnomaly = (t as unknown as { is_anomaly?: boolean }).is_anomaly ?? t.is_anomaly ?? false;
              return (
                <MobileListItem
                  key={t.id}
                  icon={<span style={{ width: 10, height: 10, borderRadius: "50%", background: catColor ?? "var(--accent)", display: "inline-block", border: isAnomaly ? "2px solid var(--negative)" : "none" }} />}
                  title={t.merchant_raw}
                  subtitle={subtitle}
                  value={formatAmount(t.amount_cents, primaryCurrency)}
                  valueTone={t.amount_cents > 0 ? "positive" : "default"}
                  meta={isAnomaly ? "Unusual" : undefined}
                  onPress={() => setDetailTxn(t)}
                />
              );
            })}
          </MobileList>

          {hasNextPage ? (
            <button
              type="button"
              onClick={() => void fetchNextPage()}
              disabled={isFetchingNextPage}
              className="btn"
              style={{ width: "100%", minHeight: 44, marginTop: 4 }}
            >
              {isFetchingNextPage ? "Loading…" : "Load more"}
            </button>
          ) : null}
        </>
      )}

      {/* Filter sheet */}
      <BottomSheet open={filterOpen} onClose={() => setFilterOpen(false)} title="Filters">
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <div>
            <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600, marginBottom: 8 }}>Preset</div>
            <SegmentedControl
              options={PRESETS as unknown as Array<{ value: string; label: string }>}
              value={preset}
              onChange={(v) => handlePresetChange(v as Preset)}
              ariaLabel="Filter preset"
            />
          </div>

          <div>
            <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600, marginBottom: 8 }}>Time range</div>
            <SegmentedControl
              options={[
                { value: "all", label: "All time" },
                { value: "month", label: "This month" },
                { value: "30d", label: "30 days" },
                { value: "week", label: "7 days" },
              ]}
              value={dateRange}
              onChange={setDateRange}
              ariaLabel="Time range"
            />
          </div>

          <div>
            <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600, marginBottom: 8 }}>Account</div>
            <select
              value={accountFilter ?? ""}
              onChange={(e) => {
                const v = e.target.value || null;
                setAccountFilter(v);
                setSearchParams((prev) => {
                  const p = new URLSearchParams(prev);
                  if (v) p.set("account", v);
                  else p.delete("account");
                  return p;
                }, { replace: true });
              }}
              style={{ width: "100%", minHeight: 44, padding: "0 12px", borderRadius: 10, border: "1px solid var(--line)", background: "var(--surface)", color: "var(--ink)" }}
            >
              <option value="">All accounts</option>
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>{a.name}</option>
              ))}
            </select>
          </div>

          <div style={{ display: "flex", gap: 10, marginTop: 8 }}>
            <button
              type="button"
              className="btn"
              style={{ flex: 1, minHeight: 44 }}
              onClick={() => {
                setPreset("all");
                setAccountFilter(null);
                setDateRange("all");
                setSearch("");
                setSearchParams(new URLSearchParams(), { replace: true });
              }}
            >
              Reset
            </button>
            <button type="button" className="btn primary" style={{ flex: 1, minHeight: 44 }} onClick={() => setFilterOpen(false)}>
              Done
            </button>
          </div>
        </div>
      </BottomSheet>

      {/* Detail sheet */}
      <BottomSheet open={!!detailTxn} onClose={() => setDetailTxn(null)} title={detailTxn?.merchant_raw ?? "Transaction"}>
        {detailTxn ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", gap: 12 }}>
              <span style={{ fontSize: 28, fontWeight: 750, letterSpacing: "-0.03em", color: detailTxn.amount_cents > 0 ? "var(--positive)" : "var(--ink)" }} className="money">
                {money(detailTxn.amount_cents, { currency: primaryCurrency })}
              </span>
              <span style={{ fontSize: 12, color: "var(--ink-mute)" }}>{new Date(detailTxn.posted_at).toLocaleDateString("en-US", { month: "long", day: "numeric", year: "numeric" })}</span>
            </div>

            <div style={{ display: "grid", gap: 10 }}>
              <div style={{ display: "flex", justifyContent: "space-between", gap: 12, padding: "10px 12px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 12 }}>
                <span style={{ color: "var(--ink-faint)", fontSize: 12, fontWeight: 600, letterSpacing: "0.06em", textTransform: "uppercase" }}>Category</span>
                <span style={{ display: "inline-flex", alignItems: "center", gap: 6, fontWeight: 600, fontSize: 13 }}>
                  <span style={{ width: 8, height: 8, borderRadius: "50%", background: detailTxn.category_color ?? "var(--line-2)", display: "inline-block" }} />
                  {(detailTxn as unknown as { category_label?: string | null }).category_label ?? detailTxn.category_label ?? "Uncategorized"}
                </span>
              </div>

              {(() => {
                const st = spendingTypeLabel(detailTxn.category_id ?? null, categories as unknown as Array<{ id: string; spendingType?: string; label: string }>);
                return st ? (
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 12, padding: "10px 12px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 12 }}>
                    <span style={{ color: "var(--ink-faint)", fontSize: 12, fontWeight: 600, letterSpacing: "0.06em", textTransform: "uppercase" }}>Type</span>
                    <span className="chip" style={{ fontSize: 12 }}>{st}</span>
                  </div>
                ) : null;
              })()}

              <div style={{ display: "flex", justifyContent: "space-between", gap: 12, padding: "10px 12px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 12 }}>
                <span style={{ color: "var(--ink-faint)", fontSize: 12, fontWeight: 600, letterSpacing: "0.06em", textTransform: "uppercase" }}>Account</span>
                <span style={{ fontWeight: 600, fontSize: 13 }}>{accountById[detailTxn.account_id]?.name ?? detailTxn.account_id.slice(0, 8)}</span>
              </div>

              <div style={{ display: "flex", justifyContent: "space-between", gap: 12, padding: "10px 12px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 12 }}>
                <span style={{ color: "var(--ink-faint)", fontSize: 12, fontWeight: 600, letterSpacing: "0.06em", textTransform: "uppercase" }}>Date</span>
                <span style={{ fontWeight: 500, fontSize: 13 }}>{new Date(detailTxn.posted_at).toLocaleString("en-US", { month: "short", day: "numeric", year: "numeric", hour: "numeric", minute: "2-digit" })}</span>
              </div>

              {detailTxn.notes ? (
                <div style={{ padding: "12px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 12 }}>
                  <div style={{ color: "var(--ink-faint)", fontSize: 11, fontWeight: 600, letterSpacing: "0.08em", textTransform: "uppercase", marginBottom: 6 }}>Notes</div>
                  <div style={{ fontSize: 13, lineHeight: 1.5, color: "var(--ink)" }}>{detailTxn.notes}</div>
                </div>
              ) : null}

              {(detailTxn as unknown as { is_anomaly?: boolean }).is_anomaly || detailTxn.is_anomaly ? (
                <div style={{ padding: "12px", background: "color-mix(in oklab, var(--negative) 9%, var(--surface))", border: "1px solid color-mix(in oklab, var(--negative) 40%, var(--line))", borderRadius: 12, color: "var(--negative)", fontSize: 13 }}>
                  Unusual charge — flagged by the detector. Review and dismiss if this is expected.
                </div>
              ) : null}

              {(detailTxn as unknown as { pending?: boolean }).pending || detailTxn.pending ? (
                <div style={{ padding: "10px 12px", background: "var(--surface-2)", border: "1px solid var(--line)", borderRadius: 12, fontSize: 12, color: "var(--ink-mute)" }}>Pending — amount may still change.</div>
              ) : null}

              {(detailTxn as unknown as { is_transfer?: boolean }).is_transfer || detailTxn.is_transfer ? (
                <div style={{ padding: "10px 12px", background: "var(--surface-2)", border: "1px solid var(--line)", borderRadius: 12, fontSize: 12, color: "var(--ink-mute)" }}>
                  Transfer {detailTxn.transfer_peer_account_name ? `→ ${detailTxn.transfer_peer_account_name}` : ""}
                </div>
              ) : null}
            </div>

            <div style={{ display: "flex", gap: 10, marginTop: 4 }}>
              <button type="button" className="btn" style={{ flex: 1, minHeight: 44 }} onClick={() => setDetailTxn(null)}>
                Close
              </button>
              <button
                type="button"
                className="btn primary"
                style={{ flex: 1, minHeight: 44 }}
                onClick={() => {
                  setEditOpen(true);
                }}
              >
                Edit
              </button>
            </div>
          </div>
        ) : null}
      </BottomSheet>

      {/* Reuse desktop drawer for editing — it already becomes a bottom sheet on mobile via CSS, so no new form needed */}
      <TransactionDrawer open={editOpen} onClose={() => { setEditOpen(false); setDetailTxn(null); }} transaction={detailTxn ?? undefined} />
    </div>
  );
}
