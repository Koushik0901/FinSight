import { open as openDialog } from "@tauri-apps/plugin-dialog";

interface Props {
  onPicked: (path: string) => void;
  label?: string;
  disabled?: boolean;
  className?: string;
}

export default function FilePicker({ onPicked, label = "Pick a CSV…", disabled, className }: Props) {
  async function pick() {
    const selected = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (typeof selected === "string") onPicked(selected);
  }
  return (
    <button type="button" className={className} onClick={pick} data-testid="file-picker" disabled={disabled}>
      {label}
    </button>
  );
}
