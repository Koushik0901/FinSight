import { useRef, useState } from "react";
import { toast } from "sonner";
import { uploadCsv } from "../api/csvUpload";
import { userErrorMessage } from "../utils/runtime";

interface Props {
  onPicked: (path: string) => void;
  label?: string;
  disabled?: boolean;
  className?: string;
}

export default function FilePicker({ onPicked, label = "Pick a CSV…", disabled, className }: Props) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [uploading, setUploading] = useState(false);

  function pick() {
    inputRef.current?.click();
  }

  async function uploadSelected(file: File | undefined) {
    if (!file) return;
    setUploading(true);
    try {
      onPicked(await uploadCsv(file));
    } catch (error) {
      toast.error("CSV upload failed", {
        description: userErrorMessage(error, "Check the file and try again."),
      });
    } finally {
      setUploading(false);
      if (inputRef.current) inputRef.current.value = "";
    }
  }

  return (
    <>
      <button
        type="button"
        className={className}
        onClick={pick}
        data-testid="file-picker"
        disabled={disabled || uploading}
      >
        {uploading ? "Uploading…" : label}
      </button>
      <input
        ref={inputRef}
        type="file"
        accept=".csv,text/csv"
        hidden
        onChange={(event) => void uploadSelected(event.target.files?.[0])}
        data-testid="csv-file-input"
      />
    </>
  );
}
