import type { ReactNode } from "react";
import { backupReminder, formatDateTime, formatToday, plural } from "../app/format";
import type { BackupStatus } from "../backup/api";
import type { SectionId } from "../app/navigation";
import type { AppStatus } from "../app/types";
import type { ReorderAlert } from "../inventory/api";
import type { StockOverview } from "../stock/api";
import { displayQuantity, movementLabel } from "../stock/quantity";
import { StatusBadge } from "./StatusBadge";
import { Callout, MovementTag } from "./ui/Feedback";
import { Icon, type IconName } from "./ui/Icon";
import { Detail, Loading, PageHeader, Panel } from "./ui/Layout";

type DashboardProps = {
  state: "loading" | "ready";
  data?: AppStatus;
  overview: StockOverview | null;
  overviewError: string | null;
  attention?: ReorderAlert[] | null;
  backupStatus?: BackupStatus | null;
  onNavigate: (section: SectionId) => void;
  onOpenMovement: (productId: number) => void;
};

const ATTENTION_LIMIT = 6;

const shortcuts: { section: SectionId; label: string; detail: string; icon: IconName }[] = [
  { section: "inventory", label: "Inventaires", detail: "Comptages physiques et validation des écarts", icon: "inventory" },
  { section: "catalogue", label: "Catalogue", detail: "Produits, références, import et export", icon: "package" },
  { section: "alerts", label: "Réapprovisionnement", detail: "Seuils et produits sous le seuil", icon: "reorder" },
  { section: "history", label: "Historique", detail: "Tous les mouvements, filtres et export", icon: "history" },
  { section: "backup", label: "Sauvegarde", detail: "Copie et restauration de la base locale", icon: "backup" },
];

type Tone = "neutral" | "warn" | "danger";

function Indicator({
  label,
  value,
  tone = "neutral",
  icon,
  hint,
  action,
}: {
  label: string;
  value: number;
  tone?: Tone;
  icon: IconName;
  hint?: string;
  action?: ReactNode;
}) {
  const accent = tone === "danger" ? "bg-danger" : tone === "warn" ? "bg-warn-accent" : "bg-transparent";
  const color = tone === "danger" ? "text-danger" : tone === "warn" ? "text-warn" : "text-ink";
  return (
    <div className="relative flex min-w-0 flex-col border-line px-4 pb-3 pt-3.5 [&:not(:last-child)]:border-r max-lg:[&:nth-child(2)]:border-r-0 max-lg:[&:nth-child(-n+2)]:border-b">
      <span className={`absolute inset-x-0 top-0 h-0.5 ${accent}`} aria-hidden="true" />
      <dt className="flex items-center justify-between gap-2 text-table font-medium text-mist">
        {label}
        <Icon name={icon} size={16} className="text-faint" />
      </dt>
      <dd className={`mt-1 text-figure font-semibold tabular-nums ${color}`}>{value}</dd>
      {hint ? <p className="mt-0.5 text-meta text-faint">{hint}</p> : null}
      {action ? <div className="mt-auto pt-2">{action}</div> : null}
    </div>
  );
}

function attentionBadge(alert: ReorderAlert) {
  if (alert.quantity === "0") {
    return <StatusBadge tone="danger">Rupture</StatusBadge>;
  }
  if (alert.gap === "0") {
    return <StatusBadge tone="warn">Au seuil</StatusBadge>;
  }
  return <StatusBadge tone="warn">Sous le seuil</StatusBadge>;
}

export function Dashboard({
  state,
  data,
  overview,
  overviewError,
  attention = null,
  backupStatus = null,
  onNavigate,
  onOpenMovement,
}: DashboardProps) {
  const zeroInList = attention ? attention.filter((alert) => alert.quantity === "0").length : 0;
  const reminder = backupStatus ? backupReminder(backupStatus.createdAt) : null;
  const rupturesWithoutThreshold = overview && attention ? Math.max(0, overview.outOfStock - zeroInList) : 0;

  return (
    <div>
      <PageHeader
        title="Tableau de bord"
        description={
          <>
            <span className="font-medium text-ink">{formatToday()}</span>
            <span className="mx-2 text-line-strong">|</span>
            Stocks, inventaires et réapprovisionnement de cet ordinateur.
          </>
        }
        actions={
          <>
            <button type="button" className="btn btn-secondary" onClick={() => onNavigate("exits")}>
              <Icon name="exit" size={16} />
              Nouvelle sortie
            </button>
            <button type="button" className="btn btn-primary" onClick={() => onNavigate("entries")}>
              <Icon name="entry" size={16} />
              Nouvelle entrée
            </button>
          </>
        }
      />

      {reminder ? (
        <Callout
          tone={reminder.tone}
          className="mb-5"
          title="Sauvegarde recommandée"
          action={
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => onNavigate("backup")}>
              <Icon name="save" size={14} />
              Créer une sauvegarde
            </button>
          }
        >
          {reminder.text} Les données n'existent que sur cet ordinateur : copiez-les sur une clé USB ou un disque
          externe.
        </Callout>
      ) : null}
      {overviewError ? (
        <Callout tone="danger" role="alert" className="mb-5">
          {overviewError}
        </Callout>
      ) : null}
      {state === "ready" && !overview && !overviewError ? <Loading>Chargement des indicateurs…</Loading> : null}

      {state === "ready" && overview ? (
        <dl className="panel mb-5 grid grid-cols-2 overflow-hidden lg:grid-cols-4">
          <Indicator
            label="Produits actifs"
            value={overview.activeProducts}
            icon="package"
            hint="Produits utilisables pour les opérations."
            action={
              <button type="button" className="link text-table" onClick={() => onNavigate("catalogue")}>
                Voir le catalogue
              </button>
            }
          />
          <Indicator
            label="Ruptures de stock"
            value={overview.outOfStock}
            tone={overview.outOfStock > 0 ? "danger" : "neutral"}
            icon="alert"
            hint="Stock disponible égal à zéro."
          />
          <Indicator
            label="À réapprovisionner"
            value={overview.reorderAlerts}
            tone={overview.reorderAlerts > 0 ? "warn" : "neutral"}
            icon="reorder"
            hint="Stock inférieur ou égal au seuil, même s'il reste du stock."
            action={
              <button type="button" className="link text-table" onClick={() => onNavigate("alerts")}>
                Voir les alertes
              </button>
            }
          />
          <Indicator
            label="Mouvements sur 7 jours"
            value={overview.movementsLast7Days}
            icon="movements"
            hint="Entrées, sorties et corrections d'inventaire."
            action={
              <button type="button" className="link text-table" onClick={() => onNavigate("history")}>
                Voir l'historique
              </button>
            }
          />
        </dl>
      ) : null}

      <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_20rem] 2xl:grid-cols-[minmax(0,1fr)_24rem]">
        <div className="min-w-0 space-y-5">
          <Panel
            title="Derniers mouvements"
            flush
            actions={
              <button type="button" className="btn btn-ghost btn-sm" onClick={() => onNavigate("history")}>
                Historique complet
                <Icon name="forward" size={14} />
              </button>
            }
          >
            {!overview ? (
              <p className="px-4 py-6 text-table text-mist">—</p>
            ) : overview.recentMovements.length === 0 ? (
              <p className="px-4 py-6 text-table text-mist">Aucun mouvement enregistré.</p>
            ) : (
              <div className="table-frame table-frame-flush rounded-b-panel">
                <table className="data-table min-w-[26rem]">
                  <thead>
                    <tr>
                      <th>Date</th>
                      <th>Produit</th>
                      <th>Mouvement</th>
                      <th className="num">Quantité</th>
                    </tr>
                  </thead>
                  <tbody>
                    {overview.recentMovements.map((movement) => (
                      <tr key={movement.id}>
                        <td className="nowrap tabular-nums text-mist">{formatDateTime(movement.createdAt)}</td>
                        <td>
                          <button type="button" className="link" onClick={() => onOpenMovement(movement.productId)}>
                            {movement.productName}
                          </button>
                          {movement.productReference ? (
                            <span className="ml-2 text-meta text-faint">{movement.productReference}</span>
                          ) : null}
                        </td>
                        <td>
                          <MovementTag
                            direction={movement.direction}
                            label={movementLabel(movement.direction, movement.source)}
                          />
                        </td>
                        <td className="num font-semibold">{displayQuantity(movement.quantity)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </Panel>

          <Panel title="Base locale" as="section">
            {state === "loading" || !data ? (
              <Loading className="py-0">Initialisation de la base locale…</Loading>
            ) : (
              <dl className="grid grid-cols-3 gap-x-6 gap-y-3 text-table">
                <Detail label="État">
                  <span className="inline-flex items-center gap-1.5 text-ok">
                    <span className="badge-dot" aria-hidden="true" />
                    <span>Initialisée</span>
                  </span>
                </Detail>
                <Detail label="Version du schéma">
                  <span className="tabular-nums">{data.schemaVersion}</span>
                </Detail>
                <Detail label="Application">{data.appName}</Detail>
                <Detail label="Fichier" className="col-span-3">
                  <span className="break-all font-normal text-mist">{data.databasePath}</span>
                </Detail>
              </dl>
            )}
          </Panel>
        </div>

        <div className="min-w-0 space-y-5">
          {attention ? (
            <Panel
              title="Articles à surveiller"
              flush
              actions={
                attention.length > 0 ? (
                  <button type="button" className="btn btn-ghost btn-sm" onClick={() => onNavigate("alerts")}>
                    Tout voir
                    <Icon name="forward" size={14} />
                  </button>
                ) : null
              }
            >
              {attention.length === 0 ? (
                <p className="flex items-center gap-2 px-4 py-4 text-table text-mist">
                  <Icon name="success" size={16} className="text-ok" />
                  Aucun produit sous son seuil.
                </p>
              ) : (
                <ul className="divide-y divide-line">
                  {attention.slice(0, ATTENTION_LIMIT).map((alert) => (
                    <li key={alert.productId} className="flex items-center gap-3 px-4 py-2.5">
                      <div className="min-w-0 flex-1">
                        <button
                          type="button"
                          className="link block max-w-full truncate text-table"
                          onClick={() => onOpenMovement(alert.productId)}
                        >
                          {alert.name}
                        </button>
                        <p className="truncate text-meta text-faint">
                          Stock <span className="font-semibold tabular-nums text-ink">{displayQuantity(alert.quantity)}</span>
                          {" · "}seuil <span className="tabular-nums">{displayQuantity(alert.threshold)}</span>
                          {alert.supplierName ? ` · ${alert.supplierName}` : ""}
                        </p>
                      </div>
                      {attentionBadge(alert)}
                    </li>
                  ))}
                </ul>
              )}
              {attention.length > ATTENTION_LIMIT || rupturesWithoutThreshold > 0 ? (
                <div className="space-y-1 border-t border-line bg-surface-muted px-4 py-2 text-meta text-mist">
                  {attention.length > ATTENTION_LIMIT ? (
                    <p>{plural(attention.length - ATTENTION_LIMIT, "autre produit", "autres produits")} sous le seuil.</p>
                  ) : null}
                  {rupturesWithoutThreshold > 0 ? (
                    <p>
                      {plural(rupturesWithoutThreshold, "rupture", "ruptures")} sans seuil configuré, visible
                      {rupturesWithoutThreshold > 1 ? "s" : ""} dans le catalogue.
                    </p>
                  ) : null}
                </div>
              ) : null}
            </Panel>
          ) : null}

          <Panel title="Accès rapides" flush>
            <ul className="divide-y divide-line">
              {shortcuts.map((shortcut) => (
                <li key={shortcut.section}>
                  <button
                    type="button"
                    className="group flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors hover:bg-surface-muted focus-visible:-outline-offset-2"
                    onClick={() => onNavigate(shortcut.section)}
                  >
                    <span className="flex size-8 shrink-0 items-center justify-center rounded-control bg-gtm-blue-soft text-gtm-blue">
                      <Icon name={shortcut.icon} size={16} />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block text-table font-medium text-ink">{shortcut.label}</span>
                      <span className="block truncate text-meta text-faint">{shortcut.detail}</span>
                    </span>
                    <Icon name="chevronRight" size={16} className="text-faint transition-transform group-hover:translate-x-0.5" />
                  </button>
                </li>
              ))}
            </ul>
          </Panel>
        </div>
      </div>
    </div>
  );
}
