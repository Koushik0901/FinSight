import { useState } from "react";
import type { ReportWidget } from "../../api/hooks/reportWidgets";
import { useCreateReportWidget, useDeleteReportWidget } from "../../api/hooks/reportWidgets";
import { toast } from "sonner";
import WidgetRenderer from "./WidgetRenderer";

type Props = {
  widget: ReportWidget;
  onEdit: (w: ReportWidget) => void;
  dragHandleProps?: React.HTMLAttributes<HTMLButtonElement>;
  isDragging?: boolean;
  memberId?: string | null;
};

export default function WidgetCard({ widget, onEdit, dragHandleProps, isDragging, memberId }: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const del = useDeleteReportWidget();
  const create = useCreateReportWidget();

  const handleDelete = async () => {
    if (!confirm(`Delete "${widget.title}"?`)) return;
    try {
      await del.mutateAsync(widget.id);
      toast.success("Widget removed", {
        description: widget.title,
      });
    } catch (e) {
      toast.error("Could not delete", { description: String(e) });
    }
    setMenuOpen(false);
  };

  const handleDuplicate = async () => {
    try {
      await create.mutateAsync({
        title: `${widget.title} (copy)`,
        chartType: widget.chartType,
        splitBy: widget.splitBy,
        period: widget.period,
        filtersJson: widget.filtersJson,
        position: null,
      });
      toast.success("Duplicated", { description: widget.title });
    } catch (e) {
      toast.error("Could not duplicate", { description: String(e) });
    }
    setMenuOpen(false);
  };

  return (
    <div
      className="card"
      style={{
        padding: 0,
        overflow: "hidden",
        opacity: isDragging ? 0.6 : 1,
        transform: isDragging ? "rotate(1deg)" : undefined,
        boxShadow: isDragging ? "0 12px 32px rgba(0,0,0,0.35)" : undefined,
        transition: "box-shadow 120ms, opacity 120ms",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          padding: "12px 14px",
          borderBottom: "1px solid var(--line)",
          background: "var(--surface-2)",
        }}
      >
        <button
          aria-label={`Drag to reorder ${widget.title}`}
          title="Drag to reorder"
          {...dragHandleProps}
          style={{
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            width: 28,
            height: 28,
            borderRadius: 8,
            border: "1px solid var(--line)",
            background: "var(--elevated)",
            cursor: "grab",
            flexShrink: 0,
          }}
        >
          <span aria-hidden style={{ display: "grid", gap: 2 }}>
            <span style={{ width: 12, height: 2, background: "var(--ink-faint)", borderRadius: 2, display: "block" }} />
            <span style={{ width: 12, height: 2, background: "var(--ink-faint)", borderRadius: 2, display: "block" }} />
            <span style={{ width: 12, height: 2, background: "var(--ink-faint)", borderRadius: 2, display: "block" }} />
          </span>
        </button>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontWeight: 650, fontSize: 14, lineHeight: 1.2, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{widget.title}</div>
          <div className="muted" style={{ fontSize: 11, marginTop: 2, display: "flex", gap: 6, flexWrap: "wrap" }}>
            <span style={{ background: "var(--elevated)", border: "1px solid var(--line)", padding: "1px 6px", borderRadius: 999 }}>{widget.chartType}</span>
            <span style={{ background: "var(--elevated)", border: "1px solid var(--line)", padding: "1px 6px", borderRadius: 999 }}>{widget.splitBy}</span>
            <span style={{ background: "var(--elevated)", border: "1px solid var(--line)", padding: "1px 6px", borderRadius: 999 }}>{widget.period}</span>
          </div>
        </div>
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <button className="btn outline sm" type="button" onClick={() => onEdit(widget)} aria-label={`Edit ${widget.title}`}>
            Edit
          </button>
          <div style={{ position: "relative" }}>
            <button
              className="btn outline sm"
              type="button"
              aria-label="More"
              aria-expanded={menuOpen}
              onClick={() => setMenuOpen((v) => !v)}
              style={{ padding: "6px 8px" }}
            >
              ⋯
            </button>
            {menuOpen && (
              <div
                role="menu"
                style={{
                  position: "absolute",
                  right: 0,
                  top: "calc(100% + 6px)",
                  minWidth: 160,
                  background: "var(--elevated)",
                  border: "1px solid var(--line)",
                  borderRadius: 12,
                  boxShadow: "0 10px 24px rgba(0,0,0,0.18)",
                  padding: 6,
                  zIndex: 5,
                  display: "flex",
                  flexDirection: "column",
                  gap: 4,
                }}
              >
                <button className="btn outline sm" type="button" role="menuitem" onClick={() => { onEdit(widget); setMenuOpen(false); }}>
                  Edit widget
                </button>
                <button className="btn outline sm" type="button" role="menuitem" onClick={handleDuplicate}>
                  Duplicate
                </button>
                <button className="btn outline sm" type="button" role="menuitem" onClick={handleDelete} style={{ color: "var(--negative)", borderColor: "var(--negative)" }}>
                  Delete
                </button>
              </div>
            )}
          </div>
        </div>
      </div>
      <div style={{ padding: 16 }}>
        <WidgetRenderer widget={widget} memberId={memberId} />
      </div>
    </div>
  );
}
