import { useState, useCallback } from "react";
import { useReportWidgets, useReorderReportWidgets } from "../../api/hooks/reportWidgets";
import type { ReportWidget } from "../../api/hooks/reportWidgets";
import WidgetCard from "./WidgetCard";
import WidgetDrawer from "./WidgetDrawer";
import EmptyState from "../EmptyState";
import { toast } from "sonner";
type Props = {
  memberId?: string | null;
};

export default function ReportCanvas({ memberId }: Props) {
  const { data: widgets = [], isLoading, error, refetch } = useReportWidgets();
  const reorder = useReorderReportWidgets();
  const [editing, setEditing] = useState<ReportWidget | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);

  const handleEdit = useCallback((w: ReportWidget) => {
    setEditing(w);
    setDrawerOpen(true);
  }, []);

  const handleAdd = useCallback(() => {
    setEditing(null);
    setDrawerOpen(true);
  }, []);

  const handleDragStart = (idx: number) => (e: React.DragEvent) => {
    setDragIndex(idx);
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", String(idx));
  };

  const handleDragOver = (idx: number) => (e: React.DragEvent) => {
    e.preventDefault();
    if (dragIndex === null || dragIndex === idx) return;
    setDragOverIndex(idx);
  };

  const handleDrop = (idx: number) => (e: React.DragEvent) => {
    e.preventDefault();
    if (dragIndex === null) return;
    const from = dragIndex;
    const to = idx;
    if (from === to) {
      setDragIndex(null);
      setDragOverIndex(null);
      return;
    }
    const ids = widgets.map((w) => w.id);
    const [moved] = ids.splice(from, 1);
    if (!moved) return;
    ids.splice(to, 0, moved);
    reorder.mutate(ids, {
      onSuccess: () => toast.success("Reordered"),
      onError: (err) => toast.error("Could not reorder", { description: String(err) }),
    });
    setDragIndex(null);
    setDragOverIndex(null);
  };

  const handleDragEnd = () => {
    setDragIndex(null);
    setDragOverIndex(null);
  };

  // Keyboard reorder fallback via buttons (inside WidgetCard menu we could add, but also provide simple)
  const moveUp = (idx: number) => {
    if (idx === 0) return;
    const ids = widgets.map((w) => w.id);
    const a = ids[idx - 1];
    const b = ids[idx];
    if (a === undefined || b === undefined) return;
    ids[idx - 1] = b;
    ids[idx] = a;
    reorder.mutate(ids);
  };
  const moveDown = (idx: number) => {
    if (idx === widgets.length - 1) return;
    const ids = widgets.map((w) => w.id);
    const a = ids[idx];
    const b = ids[idx + 1];
    if (a === undefined || b === undefined) return;
    ids[idx] = b;
    ids[idx + 1] = a;
    reorder.mutate(ids);
  };

  if (isLoading) return <div className="stub">Loading reports…</div>;
  if (error) {
    return (
      <div className="stub" role="alert">
        <p>Reports could not load.</p>
        <button className="btn outline sm" type="button" onClick={() => void refetch()}>
          Try again
        </button>
      </div>
    );
  }

  if (widgets.length === 0) {
    return (
      <>
        <EmptyState
          title="No widgets yet"
          description="Build the view you need — any slice of your ledger as a table or chart."
          actions={
            <button className="btn primary" type="button" onClick={handleAdd}>
              Add your first widget
            </button>
          }
        />
        <WidgetDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} editing={editing} />
      </>
    );
  }

  return (
    <>
      <div style={{ display: "flex", justifyContent: "flex-end", marginBottom: 12 }}>
        <button className="btn primary sm" type="button" onClick={handleAdd} style={{ gap: 6 }}>
          <span aria-hidden>+</span> Add widget
        </button>
      </div>

      <div
        role="list"
        aria-label="Report widgets"
        style={{ display: "flex", flexDirection: "column", gap: 18 }}
      >
        {widgets.map((w, idx) => (
          <div
            key={w.id}
            role="listitem"
            draggable
            onDragStart={handleDragStart(idx)}
            onDragOver={handleDragOver(idx)}
            onDrop={handleDrop(idx)}
            onDragEnd={handleDragEnd}
            style={{
              position: "relative",
              borderTop: dragOverIndex === idx && dragIndex !== null && dragIndex !== idx ? "2px solid var(--accent)" : "2px solid transparent",
              borderRadius: 12,
              transition: "border-color 120ms",
            }}
          >
            <WidgetCard
              widget={w}
              onEdit={handleEdit}
              isDragging={dragIndex === idx}
              memberId={memberId}
              dragHandleProps={{
                draggable: true,
                onDragStart: handleDragStart(idx) as unknown as React.HTMLAttributes<HTMLButtonElement>["onDragStart"],
              }}
            />
            {/* Keyboard reorder helpers — visually hidden on mobile but accessible */}
            <div style={{ display: "flex", gap: 6, justifyContent: "flex-end", marginTop: 6 }}>
              <button
                className="btn outline sm"
                type="button"
                aria-label={`Move ${w.title} up`}
                disabled={idx === 0}
                onClick={() => moveUp(idx)}
                style={{ fontSize: 11, padding: "4px 8px", opacity: idx === 0 ? 0.45 : 1 }}
              >
                ↑ Up
              </button>
              <button
                className="btn outline sm"
                type="button"
                aria-label={`Move ${w.title} down`}
                disabled={idx === widgets.length - 1}
                onClick={() => moveDown(idx)}
                style={{ fontSize: 11, padding: "4px 8px", opacity: idx === widgets.length - 1 ? 0.45 : 1 }}
              >
                ↓ Down
              </button>
            </div>
          </div>
        ))}
      </div>

      <WidgetDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} editing={editing} />
    </>
  );
}
