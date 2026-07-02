import type { ReactNode, TableHTMLAttributes } from "react";

interface TableProps extends TableHTMLAttributes<HTMLTableElement> {
  children: ReactNode;
  wrap?: boolean;
}

export default function Table({ children, wrap = true, className = "", ...rest }: TableProps) {
  const table = (
    <table className={`tbl ${className}`.trim()} {...rest}>
      {children}
    </table>
  );

  if (!wrap) return table;
  return <div className="table-wrap">{table}</div>;
}

interface TableHeadProps {
  children: ReactNode;
}

export function TableHead({ children }: TableHeadProps) {
  return <thead>{children}</thead>;
}

interface TableBodyProps {
  children: ReactNode;
}

export function TableBody({ children }: TableBodyProps) {
  return <tbody>{children}</tbody>;
}

interface TableRowProps {
  children: ReactNode;
  onClick?: () => void;
  className?: string;
  tabIndex?: number;
  onKeyDown?: (e: React.KeyboardEvent<HTMLTableRowElement>) => void;
}

export function TableRow({ children, onClick, className = "", tabIndex, onKeyDown }: TableRowProps) {
  return (
    <tr
      className={className}
      onClick={onClick}
      tabIndex={tabIndex}
      onKeyDown={onKeyDown}
      role={onClick ? "button" : undefined}
      aria-label={onClick ? "View details" : undefined}
    >
      {children}
    </tr>
  );
}

interface TableHeaderProps {
  children: ReactNode;
  right?: boolean;
  scope?: "col" | "row";
  className?: string;
}

export function TableHeader({ children, right = false, scope = "col", className = "" }: TableHeaderProps) {
  return (
    <th className={[className, right ? "right" : ""].filter(Boolean).join(" ")} scope={scope}>
      {children}
    </th>
  );
}

interface TableCellProps {
  children: ReactNode;
  right?: boolean;
  className?: string;
}

export function TableCell({ children, right = false, className = "" }: TableCellProps) {
  return <td className={[className, right ? "right" : ""].filter(Boolean).join(" ")}>{children}</td>;
}
