import { open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { toErrorMessage } from "../app/api";
import { createBackup, getBackupStatus, inspectBackup, restoreBackup } from "../backup/api";
import type { BackupReport, BackupStatus } from "../backup/api";
import { formatDateTime } from "../app/format";
import { buttonClass, quietButtonClass } from "./Field";
import { Callout, Spinner } from "./ui/Feedback";
import { Icon } from "./ui/Icon";
import { Detail, PageHeader, Panel } from "./ui/Layout";

type BackupPageProps = {
  onRestored: () => void;
  onBackupCreated?: () => void;
};

function stamp(value: string | null) {
  if (!value) {
    return "—";
  }
  return formatDateTime(value, true);
}

export function BackupPage({ onRestored, onBackupCreated }: BackupPageProps) {
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [report, setReport] = useState<BackupReport | null>(null);
  const [pendingPath, setPendingPath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const confirmRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (pendingPath) {
      confirmRef.current?.scrollIntoView?.({ block: "nearest" });
    }
  }, [pendingPath]);

  useEffect(() => {
    let cancelled = false;
    getBackupStatus()
      .then((value) => {
        if (!cancelled) {
          setStatus(value);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(toErrorMessage(reason));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [notice]);

  function begin() {
    if (busyRef.current) {
      return false;
    }
    busyRef.current = true;
    setBusy(true);
    setError(null);
    setNotice(null);
    return true;
  }

  function finish() {
    busyRef.current = false;
    setBusy(false);
  }

  async function create() {
    if (!begin()) {
      return;
    }
    try {
      const selected = await save({
        defaultPath: "sauvegarde-gtm.gtm-backup",
        filters: [{ name: "Sauvegarde GTM", extensions: ["gtm-backup"] }],
      });
      if (typeof selected !== "string" || selected.length === 0) {
        return;
      }
      const file = selected.toLowerCase().endsWith(".gtm-backup")
        ? selected
        : `${selected}.gtm-backup`;
      const created = await createBackup(file);
      setReport(created);
      setPendingPath(null);
      setNotice(`Sauvegarde enregistrée. ${file}`);
      onBackupCreated?.();
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      finish();
    }
  }

  async function inspect(forRestore: boolean) {
    if (!begin()) {
      return;
    }
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Sauvegarde GTM", extensions: ["gtm-backup", "db"] }],
      });
      const file = typeof selected === "string" ? selected : null;
      if (!file) {
        return;
      }
      const inspected = await inspectBackup(file);
      setReport(inspected);
      setPendingPath(forRestore ? file : null);
      if (!forRestore) {
        setNotice("La sauvegarde a passé les contrôles d'intégrité.");
      }
    } catch (reason) {
      setReport(null);
      setPendingPath(null);
      setError(toErrorMessage(reason));
    } finally {
      finish();
    }
  }

  function cancelRestore() {
    setPendingPath(null);
  }

  async function confirmRestore() {
    if (!pendingPath || !begin()) {
      return;
    }
    try {
      const restored = await restoreBackup(pendingPath);
      setReport(restored);
      setPendingPath(null);
      setNotice("Restauration terminée. Les données affichées ont été rechargées.");
      onRestored();
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      finish();
    }
  }

  return (
    <section>
      <PageHeader
        title="Sauvegarde et restauration"
        description="La sauvegarde enregistre une copie cohérente de la base de cet ordinateur. Elle ne quitte pas la machine et ne synchronise aucun autre poste."
      />

      <div className="panel mb-5 flex items-center gap-3 px-4 py-3">
        <span
          className={`flex size-8 shrink-0 items-center justify-center rounded-full ${
            status?.createdAt ? "bg-ok-bg text-ok" : "bg-warn-bg text-warn"
          }`}
        >
          <Icon name={status?.createdAt ? "success" : "warning"} size={16} />
        </span>
        <div className="min-w-0">
          <p className="text-table text-mist">
            Dernière sauvegarde réussie :{" "}
            {status?.createdAt ? (
              <span className="font-semibold text-ink">{stamp(status.createdAt)}</span>
            ) : (
              <span className="font-medium text-warn">aucune pour le moment.</span>
            )}
          </p>
          {status?.path ? <p className="mt-0.5 break-all text-meta text-faint">{status.path}</p> : null}
        </div>
      </div>

      <div className="mb-5 grid gap-5 md:grid-cols-2">
        <Panel title="Créer une sauvegarde" className="flex flex-col" bodyClassName="flex flex-1 flex-col">
          <p className="text-table text-mist">
            Enregistre un fichier <span className="font-medium text-ink">.gtm-backup</span> à l'emplacement de votre
            choix : clé USB, disque externe ou dossier partagé.
          </p>
          <div className="mt-auto pt-4">
            <button type="button" className={buttonClass} disabled={busy} onClick={() => void create()}>
              {busy && !pendingPath ? <Spinner /> : <Icon name="save" size={16} />}
              Créer une sauvegarde
            </button>
          </div>
        </Panel>
        <Panel title="Restaurer une sauvegarde" className="flex flex-col" bodyClassName="flex flex-1 flex-col">
          <p className="text-table text-mist">
            Vérifiez un fichier sans rien modifier, ou sélectionnez-le pour remplacer les données de ce poste. Le
            remplacement demande toujours une confirmation.
          </p>
          <div className="mt-auto flex flex-wrap gap-2 pt-4">
            <button type="button" className={quietButtonClass} disabled={busy} onClick={() => void inspect(false)}>
              <Icon name="shield" size={16} />
              Vérifier une sauvegarde
            </button>
            <button type="button" className="btn btn-danger-outline" disabled={busy} onClick={() => void inspect(true)}>
              <Icon name="restore" size={16} />
              Restaurer une sauvegarde
            </button>
          </div>
        </Panel>
      </div>

      {pendingPath && report ? (
        <div ref={confirmRef} className="panel mb-5 overflow-hidden border-danger-line">
          <div className="panel-header border-danger-line bg-danger-bg">
            <h2 className="panel-title flex items-center gap-2 text-danger">
              <Icon name="warning" size={16} />
              Confirmer la restauration
            </h2>
          </div>
          <div className="space-y-2 p-4">
            <p className="text-body">
              Restaurer cette sauvegarde remplace toutes les données actuellement présentes sur cet ordinateur. Un
              point de retour de la base actuelle est conservé avant le remplacement.
            </p>
            <p className="break-all text-table font-medium">{pendingPath}</p>
          </div>
          <div className="flex flex-wrap justify-end gap-2 rounded-b-panel border-t border-line bg-surface-muted px-4 py-3">
            <button type="button" className={quietButtonClass} disabled={busy} onClick={cancelRestore}>
              Annuler
            </button>
            <button type="button" className="btn btn-danger" disabled={busy} onClick={() => void confirmRestore()}>
              {busy ? <Spinner /> : null}
              {busy ? "Restauration…" : "Confirmer le remplacement"}
            </button>
          </div>
        </div>
      ) : null}

      {notice ? (
        <Callout tone="ok" role="status" className="mb-5" title="Opération réussie">
          <span className="break-all">{notice}</span>
        </Callout>
      ) : null}
      {error ? (
        <Callout tone="danger" role="alert" className="mb-5" title="L'opération a échoué">
          {error}
        </Callout>
      ) : null}

      {report ? (
        <Panel title="Informations sur le fichier">
          <dl className="grid grid-cols-2 gap-x-6 gap-y-3 text-table sm:grid-cols-3">
            <Detail label="Créée le">{stamp(report.createdAt)}</Detail>
            <Detail label="Application">{report.appVersion ?? report.appName}</Detail>
            <Detail label="Schéma">
              <span className="tabular-nums">{report.schemaVersion}</span>
            </Detail>
            <Detail label="Produits">
              <span className="tabular-nums">{report.productCount}</span>
            </Detail>
            <Detail label="Mouvements">
              <span className="tabular-nums">{report.movementCount}</span>
            </Detail>
            <Detail label="Inventaires">
              <span className="tabular-nums">{report.inventoryCount}</span>
            </Detail>
          </dl>
        </Panel>
      ) : null}
    </section>
  );
}
