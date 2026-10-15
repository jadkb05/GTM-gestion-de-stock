import { useEffect, useState } from "react";
import { toErrorMessage } from "../app/api";
import { setReorderThreshold } from "../inventory/api";
import { getStockMovement, getStockSheet, listStockSheetMovements } from "../stock/api";
import type { SheetLine, StockMovement, StockSheet as StockSheetData } from "../stock/api";
import { displayQuantity, movementLabel, parseNonNegative } from "../stock/quantity";
import { formatDateTime } from "../app/format";
import { buttonClass, controlClass, FilterField, quietButtonClass } from "./Field";
import { MovementDetailDialog } from "./MovementDetailDialog";
import { StatusBadge } from "./StatusBadge";
import { Callout, MovementTag } from "./ui/Feedback";
import { Icon } from "./ui/Icon";
import { Detail, Loading, PageHeader, Panel } from "./ui/Layout";

type StockSheetProps = {
  productId: number;
  onBack: () => void;
  backLabel: string;
  onStockChanged?: () => void;
};

function label(line: { direction: "in" | "out"; source?: string }) {
  return movementLabel(line.direction, line.source);
}

export function StockSheet({ productId, onBack, backLabel, onStockChanged }: StockSheetProps) {
  const [sheet, setSheet] = useState<StockSheetData | null>(null);
  const [lines, setLines] = useState<SheetLine[]>([]);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(50);
  const [total, setTotal] = useState(0);
  const [direction, setDirection] = useState<"all" | "in" | "out">("all");
  const [search, setSearch] = useState("");
  const [fromDate, setFromDate] = useState("");
  const [toDate, setToDate] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [detail, setDetail] = useState<StockMovement | null>(null);
  const [threshold, setThreshold] = useState("");
  const [thresholdError, setThresholdError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setDetail(null);
    const periodInvalid = fromDate !== "" && toDate !== "" && toDate < fromDate;
    const sheetRequest = getStockSheet(productId);
    const pageRequest = periodInvalid
      ? Promise.resolve(null)
      : listStockSheetMovements({
          productId,
          direction,
          search,
          fromDate,
          toDate,
          page,
        });
    Promise.all([sheetRequest, pageRequest])
      .then(([nextSheet, nextPage]) => {
        if (cancelled) {
          return;
        }
        setSheet(nextSheet);
        setThreshold(nextSheet.reorderThreshold ? displayQuantity(nextSheet.reorderThreshold) : "");
        if (periodInvalid || !nextPage) {
          setLines([]);
          setTotal(0);
          setError("La date de fin précède la date de début.");
        } else {
          setLines(nextPage.lines);
          setPageSize(nextPage.pageSize);
          setTotal(nextPage.total);
        }
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(toErrorMessage(reason));
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [productId, direction, search, fromDate, toDate, page, revision]);

  function changeFilter(update: () => void) {
    update();
    setPage(1);
  }

  const filtered = search.trim() !== "" || direction !== "all" || fromDate !== "" || toDate !== "";
  const shownFrom = total === 0 ? 0 : (page - 1) * pageSize + 1;
  const shownTo = total === 0 ? 0 : (page - 1) * pageSize + lines.length;
  const pageCount = Math.max(1, Math.ceil(total / pageSize));

  async function openDetail(id: number) {
    try {
      setDetail(await getStockMovement(id));
    } catch (reason) {
      setError(toErrorMessage(reason));
    }
  }

  async function saveThreshold() {
    const raw = threshold.trim();
    const parsed = parseNonNegative(raw);
    if ("error" in parsed) {
      setThresholdError(parsed.error.replace("quantité", "seuil").replace("La quantité", "Le seuil"));
      return;
    }
    setThresholdError(null);
    try {
      await setReorderThreshold(productId, raw.replace(",", "."));
      setRevision((current) => current + 1);
      onStockChanged?.();
    } catch (reason) {
      setThresholdError(toErrorMessage(reason));
    }
  }

  async function clearThreshold() {
    setThresholdError(null);
    try {
      await setReorderThreshold(productId, null);
      setThreshold("");
      setRevision((current) => current + 1);
      onStockChanged?.();
    } catch (reason) {
      setThresholdError(toErrorMessage(reason));
    }
  }

  const quantityMilli = sheet ? parseNonNegative(sheet.quantity) : null;
  const thresholdMilli = sheet?.reorderThreshold ? parseNonNegative(sheet.reorderThreshold) : null;
  const rupture = Boolean(sheet?.isActive && sheet.quantity === "0");
  const belowThreshold = Boolean(
    sheet?.isActive &&
      quantityMilli &&
      thresholdMilli &&
      "milli" in quantityMilli &&
      "milli" in thresholdMilli &&
      quantityMilli.milli <= thresholdMilli.milli,
  );

  return (
    <section>
      {!sheet ? (
        <PageHeader title="Fiche de stock" back={{ label: backLabel, onClick: onBack }} />
      ) : (
        <PageHeader
          title={sheet.name}
          back={{ label: backLabel, onClick: onBack }}
          meta={
            <>
              <StatusBadge tone={sheet.isActive ? "ok" : "neutral"}>{sheet.isActive ? "Actif" : "Désactivé"}</StatusBadge>
              {rupture ? <StatusBadge tone="danger">Rupture de stock</StatusBadge> : null}
              {!rupture && belowThreshold ? <StatusBadge tone="warn">Sous le seuil de réapprovisionnement</StatusBadge> : null}
            </>
          }
          description="Fiche de stock : situation, seuil et mouvements du produit."
        />
      )}
      {loading && !sheet ? <Loading>Chargement de la fiche…</Loading> : null}
      {error && !sheet ? (
        <Callout tone="danger" role="alert">
          {error}
        </Callout>
      ) : null}
      {sheet ? (
        <>
          {!sheet.isActive ? (
            <Callout tone="neutral" className="mb-4">
              Ce produit est désactivé. La fiche et l'historique restent consultables.
            </Callout>
          ) : null}
          <div className="mb-5 grid items-start gap-5 lg:grid-cols-[minmax(0,1fr)_20rem]">
            <div className="panel min-w-0">
              <dl className="grid grid-cols-3 gap-x-6 gap-y-3 border-b border-line px-4 py-3 text-table">
                <Detail label="Référence">{sheet.reference ?? "—"}</Detail>
                <Detail label="Type">{sheet.typeName}</Detail>
                <Detail label="Fournisseur">{sheet.supplierName ?? "—"}</Detail>
              </dl>
              <dl className="grid grid-cols-2 sm:grid-cols-4">
                <div className={`border-line px-4 py-3 sm:border-r ${rupture ? "bg-danger-bg" : "bg-gtm-blue-soft/60"}`}>
                  <dt className="text-table font-medium text-mist">Stock actuel</dt>
                  <dd className={`mt-0.5 text-figure font-semibold tabular-nums ${rupture ? "text-danger" : "text-gtm-blue"}`}>
                    {displayQuantity(sheet.quantity)}
                  </dd>
                </div>
                <div className="border-line px-4 py-3 sm:border-r">
                  <dt className="text-table text-mist">Total des entrées</dt>
                  <dd className="mt-0.5 text-[1.25rem] font-semibold tabular-nums">{displayQuantity(sheet.entriesTotal)}</dd>
                </div>
                <div className="border-line px-4 py-3 sm:border-r">
                  <dt className="text-table text-mist">Total des sorties</dt>
                  <dd className="mt-0.5 text-[1.25rem] font-semibold tabular-nums">{displayQuantity(sheet.exitsTotal)}</dd>
                </div>
                <div className="px-4 py-3">
                  <dt className="text-table text-mist">Mouvements</dt>
                  <dd className="mt-0.5 text-[1.25rem] font-semibold tabular-nums">{sheet.movementCount}</dd>
                </div>
              </dl>
            </div>
            <form
              className="panel"
              onSubmit={(event) => {
                event.preventDefault();
                void saveThreshold();
              }}
            >
              <div className="panel-header">
                <h2 className="panel-title">Seuil de réapprovisionnement</h2>
              </div>
              <div className="space-y-3 p-4">
                <p className="text-table text-mist">
                  {sheet.reorderThreshold
                    ? `Seuil actuel : ${displayQuantity(sheet.reorderThreshold)}.`
                    : "Aucun seuil. Ce produit ne participe pas aux alertes."}
                </p>
                <label className="block">
                  <span className="field-label">Nouveau seuil</span>
                  <input
                    aria-label="Seuil de réapprovisionnement"
                    value={threshold}
                    inputMode="decimal"
                    aria-invalid={Boolean(thresholdError)}
                    onChange={(event) => setThreshold(event.target.value)}
                    className={`${controlClass} control-num`}
                  />
                </label>
                {thresholdError ? (
                  <Callout tone="danger" role="alert">
                    {thresholdError}
                  </Callout>
                ) : null}
                <div className="flex flex-wrap gap-2">
                  <button type="submit" className={`${buttonClass} btn-sm`}>
                    Enregistrer le seuil
                  </button>
                  <button type="button" className="btn btn-ghost btn-sm" onClick={() => void clearThreshold()}>
                    Désactiver le seuil
                  </button>
                </div>
              </div>
            </form>
          </div>

          <Panel title="Mouvements du produit" flush>
            <div className="grid grid-cols-[repeat(2,minmax(0,0.8fr))_minmax(0,1fr)_minmax(0,1.4fr)] items-end gap-3 border-b border-line bg-surface-muted px-4 py-3">
              <FilterField label="Du">
                <input
                  type="date"
                  value={fromDate}
                  onChange={(event) => changeFilter(() => setFromDate(event.target.value))}
                  className={`${controlClass} control-sm`}
                />
              </FilterField>
              <FilterField label="Au">
                <input
                  type="date"
                  value={toDate}
                  onChange={(event) => changeFilter(() => setToDate(event.target.value))}
                  className={`${controlClass} control-sm`}
                />
              </FilterField>
              <FilterField label="Mouvement">
                <select
                  value={direction}
                  onChange={(event) =>
                    changeFilter(() => setDirection(event.target.value as "all" | "in" | "out"))
                  }
                  className={`${controlClass} control-sm`}
                >
                  <option value="all">Entrées et sorties</option>
                  <option value="in">Entrées</option>
                  <option value="out">Sorties</option>
                </select>
              </FilterField>
              <FilterField label="Motif ou référence">
                <input
                  value={search}
                  onChange={(event) => changeFilter(() => setSearch(event.target.value))}
                  className={`${controlClass} control-sm`}
                />
              </FilterField>
            </div>
            {error ? (
              <div className="p-4">
                <Callout tone="danger" role="alert">
                  {error}
                </Callout>
              </div>
            ) : null}
            {loading ? <Loading className="px-4">Chargement des mouvements…</Loading> : null}
            {!loading && !error && lines.length === 0 ? (
              <p className="px-4 py-8 text-center text-table text-mist">
                {filtered || sheet.movementCount > 0
                  ? "Aucun mouvement ne correspond à ces critères."
                  : "Aucun mouvement n'est enregistré pour ce produit."}
              </p>
            ) : null}
            {!error && lines.length > 0 ? (
              <>
                <div className="table-frame table-frame-flush max-h-[32rem]">
                  <table className="data-table min-w-[56rem]">
                    <thead>
                      <tr>
                        <th>Date</th>
                        <th>Mouvement</th>
                        <th className="num">N°</th>
                        <th className="num">Quantité entrée</th>
                        <th className="num">Quantité sortie</th>
                        <th className="num">Solde après</th>
                        <th>Motif</th>
                        <th>Référence documentaire</th>
                        <th className="actions">Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {lines.map((line) => (
                        <tr key={line.id}>
                          <td className="nowrap tabular-nums text-mist">{formatDateTime(line.createdAt)}</td>
                          <td>
                            <MovementTag direction={line.direction} label={label(line)} />
                          </td>
                          <td className="num text-faint">{line.id}</td>
                          <td className="num text-ok">
                            {line.direction === "in" ? displayQuantity(line.quantity) : <span className="text-line-strong">—</span>}
                          </td>
                          <td className="num text-[#7a3d00]">
                            {line.direction === "out" ? displayQuantity(line.quantity) : <span className="text-line-strong">—</span>}
                          </td>
                          <td className="num font-semibold">{displayQuantity(line.balanceAfter)}</td>
                          <td className="max-w-[14rem] truncate" title={line.reason ?? undefined}>
                            {line.reason ?? <span className="text-faint">—</span>}
                          </td>
                          <td className="ref">{line.documentReference ?? "—"}</td>
                          <td className="actions">
                            <button type="button" className="btn btn-ghost btn-sm" onClick={() => void openDetail(line.id)}>
                              Détail
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
                <div className="flex flex-wrap items-center justify-between gap-3 border-t border-line px-4 py-2.5">
                  <p className="text-meta text-mist">
                    {shownFrom}–{shownTo} sur {total} mouvements. Page {page} sur {pageCount}.
                  </p>
                  <div className="flex gap-2">
                    <button
                      type="button"
                      className={`${quietButtonClass} btn-sm`}
                      disabled={page <= 1}
                      onClick={() => setPage((current) => current - 1)}
                    >
                      <Icon name="chevronLeft" size={14} />
                      Page précédente
                    </button>
                    <button
                      type="button"
                      className={`${quietButtonClass} btn-sm`}
                      disabled={page >= pageCount}
                      onClick={() => setPage((current) => current + 1)}
                    >
                      Page suivante
                      <Icon name="chevronRight" size={14} />
                    </button>
                  </div>
                </div>
              </>
            ) : null}
          </Panel>
          {detail ? <MovementDetailDialog movement={detail} onClose={() => setDetail(null)} /> : null}
        </>
      ) : null}
    </section>
  );
}
