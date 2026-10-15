import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { toErrorMessage } from "../app/api";
import type { ExportFormat, ExportOutcome } from "../exchange/api";
import { quietButtonClass } from "./Field";
import { Callout } from "./ui/Feedback";
import { Icon } from "./ui/Icon";

const labels: Record<ExportFormat, string> = {
  csv: "CSV",
  xlsx: "Excel",
  pdf: "PDF",
};

type ExportBarProps = {
  stem: string;
  scope: string;
  beforeSave?: () => string | null;
  onExport: (path: string, format: ExportFormat) => Promise<ExportOutcome>;
};

export function ExportBar({ stem, scope, beforeSave, onExport }: ExportBarProps) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open && !notice && !error) {
      return;
    }
    function onPointer(event: MouseEvent) {
      if (root.current && !root.current.contains(event.target as Node)) {
        setOpen(false);
      }
    }
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [open, notice, error]);

  async function choose(format: ExportFormat) {
    const blocked = beforeSave?.();
    if (blocked) {
      setNotice(null);
      setError(blocked);
      return;
    }
    setError(null);
    setNotice(null);
    const selected = await save({
      defaultPath: `${stem}.${format === "xlsx" ? "xlsx" : format}`,
      filters: [{ name: labels[format], extensions: [format === "xlsx" ? "xlsx" : format] }],
    });
    if (typeof selected !== "string" || selected.length === 0) {
      return;
    }
    setBusy(true);
    try {
      const outcome = await onExport(selected, format);
      const lines = outcome.rows === 1 ? "1 ligne" : `${outcome.rows} lignes`;
      setNotice(`Fichier enregistré. ${lines}. ${outcome.scope}`);
      setOpen(false);
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="relative" ref={root}>
      <button
        type="button"
        className={quietButtonClass}
        disabled={busy}
        aria-expanded={open}
        onClick={() => {
          setOpen((value) => !value);
          setNotice(null);
          setError(null);
        }}
      >
        <Icon name="download" size={16} />
        Exporter
      </button>
      {open ? (
        <div className="panel absolute right-0 top-full z-30 mt-1.5 w-[23rem] p-3 shadow-[0_8px_24px_rgb(17_24_31/0.14)]">
          <p className="text-table text-ink">{scope}</p>
          <p className="mt-1.5 text-meta text-faint">
            Le CSV est en UTF-8, avec un point-virgule et une virgule décimale. Les quantités viennent des millièmes
            entiers, sans nombre binaire.
          </p>
          <div className="mt-3 grid grid-cols-3 gap-2">
            {(Object.keys(labels) as ExportFormat[]).map((format) => (
              <button
                key={format}
                type="button"
                className={quietButtonClass}
                disabled={busy}
                onClick={() => void choose(format)}
              >
                {busy ? "Export…" : labels[format]}
              </button>
            ))}
          </div>
          {error ? (
            <Callout tone="danger" role="alert" className="mt-3">
              {error}
            </Callout>
          ) : null}
        </div>
      ) : null}
      {!open && (notice || error) ? (
        <div className="absolute right-0 top-full z-30 mt-1.5 w-[23rem] shadow-[0_8px_24px_rgb(17_24_31/0.14)]">
          <Callout
            tone={error ? "danger" : "ok"}
            role={error ? "alert" : "status"}
            action={
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => {
                  setNotice(null);
                  setError(null);
                }}
              >
                Fermer
              </button>
            }
          >
            {error ?? notice}
          </Callout>
        </div>
      ) : null}
    </div>
  );
}
