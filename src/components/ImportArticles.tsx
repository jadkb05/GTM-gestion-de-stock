import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { toErrorMessage } from "../app/api";
import { commitArticleImport, previewArticleImport } from "../exchange/api";
import type { ImportPreview } from "../exchange/api";
import { buttonClass, quietButtonClass } from "./Field";
import { StatusBadge } from "./StatusBadge";
import { Dialog } from "./ui/Dialog";
import { Callout, Spinner } from "./ui/Feedback";
import { Icon } from "./ui/Icon";

export const IMPORT_COLUMNS_HINT =
  "Fichier CSV ou Excel (.xlsx) avec une ligne d'en-tête : colonnes « Nom » et « Type » obligatoires, « Référence », « Fournisseur » et « Description » facultatives. Les types et fournisseurs doivent déjà exister. Le stock n'est pas importé : saisissez-le ensuite par un inventaire.";

type ImportArticlesProps = {
  onImported: () => void;
};

export function ImportArticles({ onImported }: ImportArticlesProps) {
  const [path, setPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function chooseFile() {
    setNotice(null);
    setError(null);
    const selected = await open({
      multiple: false,
      filters: [{ name: "Articles CSV ou Excel", extensions: ["csv", "xlsx"] }],
    });
    const file = typeof selected === "string" ? selected : null;
    if (!file) {
      return;
    }
    setBusy(true);
    try {
      setPath(file);
      setPreview(await previewArticleImport(file));
    } catch (reason) {
      setPath(null);
      setPreview(null);
      setError(toErrorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  function cancel() {
    setPath(null);
    setPreview(null);
    setError(null);
  }

  async function confirm() {
    if (!path || !preview || preview.issues.length > 0 || preview.ready.length === 0 || busy) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const result = await commitArticleImport(path, preview.fingerprint);
      setNotice(
        result.created === 1
          ? "1 article importé. Le stock n'a pas été modifié."
          : `${result.created} articles importés. Le stock n'a pas été modifié.`,
      );
      setPath(null);
      setPreview(null);
      onImported();
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  const blocked = !preview || preview.issues.length > 0 || preview.ready.length === 0;

  return (
    <div className="relative">
      <button
        type="button"
        className={quietButtonClass}
        disabled={busy}
        title={IMPORT_COLUMNS_HINT}
        onClick={() => void chooseFile()}
      >
        {busy && !preview ? <Spinner /> : <Icon name="upload" size={16} />}
        Importer
      </button>
      {preview && path ? (
        <Dialog
          title="Aperçu de l'import d'articles"
          size="lg"
          onClose={cancel}
          closeDisabled={busy}
          footer={
            <>
              <p className="mr-auto text-meta text-mist">
                {preview.issues.length > 0
                  ? "Corrigez le fichier. Aucune ligne ne sera enregistrée."
                  : preview.ready.length > 0
                    ? "La confirmation ajoute ces articles. Le stock reste inchangé."
                    : "Aucune ligne à importer."}
              </p>
              <button type="button" className={quietButtonClass} disabled={busy} onClick={cancel}>
                Annuler
              </button>
              <button type="button" className={buttonClass} disabled={busy || blocked} onClick={() => void confirm()}>
                {busy ? <Spinner /> : null}
                {busy ? "Import…" : "Confirmer l'import"}
              </button>
            </>
          }
        >
          <dl className="mb-3 grid gap-1 text-table">
            <div className="flex gap-2">
              <dt className="w-20 shrink-0 text-mist">Fichier</dt>
              <dd className="min-w-0 break-all font-medium">{path}</dd>
            </div>
            <div className="flex items-center gap-2">
              <dt className="w-20 shrink-0 text-mist">Contrôle</dt>
              <dd className="flex flex-wrap gap-2">
                <StatusBadge tone={preview.ready.length > 0 ? "ok" : "neutral"}>
                  {preview.ready.length === 1 ? "1 article prêt" : `${preview.ready.length} articles prêts`}
                </StatusBadge>
                {preview.issues.length > 0 ? (
                  <StatusBadge tone="danger">
                    {preview.issues.length === 1 ? "1 anomalie" : `${preview.issues.length} anomalies`}
                  </StatusBadge>
                ) : null}
              </dd>
            </div>
          </dl>
          <p className="text-table text-mist">
            L'import ajoute des articles. Il ne crée pas de type ni de fournisseur, et il ne modifie pas le stock.
          </p>
          <p className="mt-1 text-meta text-faint">{IMPORT_COLUMNS_HINT}</p>
          {preview.ignoredColumns.length > 0 ? (
            <p className="mt-1 text-table text-mist">Colonnes ignorées : {preview.ignoredColumns.join(", ")}.</p>
          ) : null}
          {preview.issues.length > 0 ? (
            <Callout tone="danger" className="mt-3" title="Anomalies à corriger">
              <ul className="list-disc space-y-0.5 pl-4">
                {preview.issues.map((issue) => (
                  <li key={`${issue.line}-${issue.message}`}>
                    {issue.line > 0 ? `Ligne ${issue.line} : ` : ""}
                    {issue.message}
                  </li>
                ))}
              </ul>
            </Callout>
          ) : null}
          {preview.ready.length > 0 ? (
            <div className="table-frame mt-3 max-h-72">
              <table className="data-table min-w-[32rem]">
                <thead>
                  <tr>
                    <th className="num">Ligne</th>
                    <th>Nom</th>
                    <th>Référence</th>
                    <th>Type</th>
                    <th>Fournisseur</th>
                  </tr>
                </thead>
                <tbody>
                  {preview.ready.slice(0, 8).map((row) => (
                    <tr key={row.line}>
                      <td className="num text-faint">{row.line}</td>
                      <td className="font-medium">{row.name}</td>
                      <td className="ref">{row.reference ?? ""}</td>
                      <td>{row.typeName}</td>
                      <td>{row.supplierName ?? ""}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : null}
          {preview.ready.length > 8 ? (
            <p className="mt-2 text-meta text-faint">
              Aperçu des 8 premières lignes. {preview.ready.length} articles prêts.
            </p>
          ) : null}
          {error ? (
            <Callout tone="danger" role="alert" className="mt-3">
              {error}
            </Callout>
          ) : null}
        </Dialog>
      ) : null}
      {!preview && (notice || error) ? (
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
