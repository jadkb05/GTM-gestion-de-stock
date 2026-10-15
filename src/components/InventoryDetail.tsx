import { useEffect, useState } from "react";
import { toErrorMessage } from "../app/api";
import { listProductTypes, listProducts } from "../catalogue/api";
import type { NamedRecord, Product } from "../catalogue/types";
import {
  addInventoryProducts,
  getInventory,
  refreshInventoryLine,
  removeInventoryLine,
  saveInventoryCounts,
  validateInventory,
} from "../inventory/api";
import type { InventoryDetail as InventoryData, InventoryLine } from "../inventory/api";
import { displayQuantity, parseNonNegative } from "../stock/quantity";
import { exportInventory } from "../exchange/api";
import { ExportBar } from "./ExportBar";
import { formatDateTime } from "../app/format";
import { buttonClass, controlClass, FilterField, quietButtonClass } from "./Field";
import { StatusBadge, type BadgeTone } from "./StatusBadge";
import { Dialog } from "./ui/Dialog";
import { Callout, Spinner } from "./ui/Feedback";
import { Icon } from "./ui/Icon";
import { Loading, PageHeader } from "./ui/Layout";

type InventoryDetailProps = {
  inventoryId: number;
  onBack: () => void;
  onOpenSheet: (productId: number) => void;
  onStockChanged: () => void;
};

function statusLabel(status: string) {
  return status === "validated" ? "Validé" : "Brouillon";
}

function varianceText(line: InventoryLine) {
  if (line.drifted) {
    return "Référence périmée";
  }
  if (line.variance === null) {
    return "Non compté";
  }
  if (line.variance === "0") {
    return "Écart nul";
  }
  if (line.variance.startsWith("-")) {
    return `Manque de ${displayQuantity(line.variance.slice(1))}`;
  }
  return `Surplus de ${displayQuantity(line.variance)}`;
}

function varianceTone(line: InventoryLine): BadgeTone {
  if (line.drifted) {
    return "warn";
  }
  if (line.variance === null) {
    return "neutral";
  }
  if (line.variance === "0") {
    return "ok";
  }
  return line.variance.startsWith("-") ? "danger" : "info";
}

const toneDot = {
  info: "bg-gtm-blue",
  ok: "bg-ok",
  warn: "bg-warn-accent",
  danger: "bg-danger",
  neutral: "bg-line-strong",
} as const;

const toneText = {
  info: "text-ink",
  ok: "text-ink",
  warn: "text-warn",
  danger: "text-danger",
  neutral: "text-ink",
} as const;

// La validation applique les quantités enregistrées, pas celles des champs.
// Une saisie différente de la valeur enregistrée doit donc d'abord être enregistrée.
function sameCount(draft: string, counted: string | null) {
  const raw = draft.trim();
  if (raw.length === 0 || counted === null) {
    return raw.length === 0 && counted === null;
  }
  const typed = parseNonNegative(raw);
  const saved = parseNonNegative(counted);
  return "milli" in typed && "milli" in saved && typed.milli === saved.milli;
}

function unsavedLines(lines: InventoryLine[], drafts: Record<number, string>) {
  return lines.filter((line) => !sameCount(drafts[line.id] ?? "", line.counted)).length;
}

function draftsFrom(lines: InventoryLine[]) {
  const drafts: Record<number, string> = {};
  for (const line of lines) {
    drafts[line.id] = line.counted === null ? "" : displayQuantity(line.counted);
  }
  return drafts;
}

export function InventoryDetail({ inventoryId, onBack, onOpenSheet, onStockChanged }: InventoryDetailProps) {
  const [detail, setDetail] = useState<InventoryData | null>(null);
  const [drafts, setDrafts] = useState<Record<number, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [adding, setAdding] = useState(false);
  const [products, setProducts] = useState<Product[]>([]);
  const [types, setTypes] = useState<NamedRecord[]>([]);
  const [search, setSearch] = useState("");
  const [typeId, setTypeId] = useState("");
  const [selected, setSelected] = useState<number[]>([]);
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setConfirming(false);
    getInventory(inventoryId)
      .then((next) => {
        if (cancelled) {
          return;
        }
        setDetail(next);
        setDrafts(draftsFrom(next.lines));
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
  }, [inventoryId]);

  async function openPicker() {
    setError(null);
    try {
      const [nextProducts, nextTypes] = await Promise.all([
        listProducts({ search: "", typeId: "", supplierId: "", status: "active" }),
        listProductTypes(false),
      ]);
      setProducts(nextProducts);
      setTypes(nextTypes);
      setSelected([]);
      setAdding(true);
    } catch (reason) {
      setError(toErrorMessage(reason));
    }
  }

  function applyDetail(next: InventoryData) {
    setDetail(next);
    setDrafts(draftsFrom(next.lines));
  }

  async function addSelected() {
    if (!detail || selected.length === 0) {
      setError("Sélectionnez au moins un produit.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const next = await addInventoryProducts(detail.id, selected);
      setAdding(false);
      setSelected([]);
      applyDetail(next);
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setSaving(false);
    }
  }

  async function saveCounts() {
    if (!detail) {
      return;
    }
    const counts: { lineId: number; quantity: string | null }[] = [];
    for (const line of detail.lines) {
      const raw = (drafts[line.id] ?? "").trim();
      if (raw.length === 0) {
        counts.push({ lineId: line.id, quantity: null });
        continue;
      }
      const parsed = parseNonNegative(raw);
      if ("error" in parsed) {
        setError(`${line.productName} : ${parsed.error}`);
        return;
      }
      counts.push({ lineId: line.id, quantity: raw.replace(",", ".") });
    }
    setSaving(true);
    setError(null);
    try {
      applyDetail(await saveInventoryCounts(detail.id, counts));
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setSaving(false);
    }
  }

  async function refresh(lineId: number, resetCount: boolean) {
    setSaving(true);
    setError(null);
    try {
      applyDetail(await refreshInventoryLine(lineId, resetCount));
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setSaving(false);
    }
  }

  async function remove(lineId: number) {
    setSaving(true);
    setError(null);
    try {
      applyDetail(await removeInventoryLine(lineId));
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setSaving(false);
    }
  }

  async function confirmValidation() {
    if (!detail) {
      return;
    }
    if (unsavedLines(detail.lines, drafts) > 0) {
      setError("Des quantités saisies ne sont pas enregistrées. Enregistrez-les avant de valider.");
      setConfirming(false);
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const next = await validateInventory(detail.id);
      applyDetail(next);
      setConfirming(false);
      onStockChanged();
    } catch (reason) {
      setError(toErrorMessage(reason));
      setConfirming(false);
    } finally {
      setSaving(false);
    }
  }

  const locked = detail?.status === "validated";
  const counts = detail?.counts;
  const unsaved = detail && !locked ? unsavedLines(detail.lines, drafts) : 0;
  const canValidate = Boolean(
    detail &&
      !locked &&
      detail.lines.length > 0 &&
      counts &&
      counts.uncounted === 0 &&
      counts.stale === 0 &&
      unsaved === 0,
  );
  const present = new Set(detail?.lines.map((line) => line.productId) ?? []);
  const visibleProducts = products.filter((product) => {
    if (present.has(product.id)) {
      return false;
    }
    if (typeId && String(product.typeId) !== typeId) {
      return false;
    }
    const needle = search.trim().toLowerCase();
    if (!needle) {
      return true;
    }
    return (
      product.name.toLowerCase().includes(needle) ||
      (product.reference ?? "").toLowerCase().includes(needle)
    );
  });

  const total = detail?.lines.length ?? 0;
  const progress = total === 0 || !counts ? 0 : Math.round((counts.counted / total) * 100);
  const blockers: string[] = [];
  if (detail && !locked) {
    if (unsaved > 0) {
      blockers.push(
        unsaved === 1
          ? "1 quantité saisie n'est pas enregistrée."
          : `${unsaved} quantités saisies ne sont pas enregistrées.`,
      );
    }
    if (detail.lines.length === 0) {
      blockers.push("Ajoutez au moins un produit.");
    }
    if (counts && counts.uncounted > 0) {
      blockers.push(counts.uncounted === 1 ? "1 produit reste à compter." : `${counts.uncounted} produits restent à compter.`);
    }
    if (counts && counts.stale > 0) {
      blockers.push(
        counts.stale === 1 ? "1 référence périmée à traiter." : `${counts.stale} références périmées à traiter.`,
      );
    }
  }

  function focusNextCount(lineId: number) {
    const inputs = Array.from(document.querySelectorAll<HTMLInputElement>("input[data-count-line]"));
    const index = inputs.findIndex((input) => input.dataset.countLine === String(lineId));
    inputs[index + 1]?.focus();
    inputs[index + 1]?.select();
  }

  return (
    <section>
      <PageHeader
        back={{ label: "Retour aux inventaires", onClick: onBack }}
        title={detail ? (detail.name ?? `Inventaire ${detail.id}`) : "Inventaire"}
        meta={
          detail ? (
            <StatusBadge tone={locked ? "ok" : "info"}>{statusLabel(detail.status)}</StatusBadge>
          ) : null
        }
        description={
          detail ? (
            <>
              Créé le {formatDateTime(detail.createdAt)}
              {detail.validatedAt ? ` · validé le ${formatDateTime(detail.validatedAt)}` : ""}
              {detail.comment ? ` — ${detail.comment}` : ""}
            </>
          ) : null
        }
        actions={
          detail ? (
            <ExportBar
              stem={`inventaire-${detail.id}`}
              scope="L'export contient toutes les lignes de cet inventaire, y compris celles qui ne sont pas comptées. L'écart utilise le stock de référence figé."
              onExport={(path, format) => exportInventory(path, format, detail.id)}
            />
          ) : null
        }
      />
      {loading && !detail ? <Loading>Chargement de l'inventaire…</Loading> : null}
      {error && !adding ? (
        <Callout tone="danger" role="alert" className="mb-4">
          {error}
        </Callout>
      ) : null}
      {detail ? (
        <>
          {locked ? (
            <Callout tone="ok" className="mb-4">
              Inventaire validé. Il n'est plus modifiable.
            </Callout>
          ) : null}

          {counts ? (
            <div className="panel mb-4">
              <div className="flex flex-wrap items-center gap-x-6 gap-y-2 border-b border-line px-4 py-3">
                <div className="min-w-[14rem] flex-1">
                  <div className="flex items-baseline justify-between gap-3 text-table">
                    <span className="font-medium">Progression du comptage</span>
                    <span className="tabular-nums text-mist">
                      {counts.counted} / {total} · {progress} %
                    </span>
                  </div>
                  <div
                    className="mt-1.5 h-2 overflow-hidden rounded-full bg-surface-sunken"
                    role="progressbar"
                    aria-label="Progression du comptage"
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={progress}
                  >
                    <div
                      className={`h-full rounded-full transition-[width] ${progress === 100 ? "bg-ok" : "bg-gtm-blue"}`}
                      style={{ width: `${progress}%` }}
                    />
                  </div>
                </div>
                <p className="max-w-md text-meta text-faint">
                  Le stock de référence est figé au moment où le produit est ajouté à l'inventaire. Un mouvement
                  ultérieur ne modifie pas cet instantané.
                </p>
              </div>
              <dl className="grid grid-cols-3 xl:grid-cols-6">
                {(
                  [
                    ["Produits comptés", counts.counted, "info"],
                    ["Sans écart", counts.unchanged, "ok"],
                    ["Surplus", counts.surplus, "info"],
                    ["Manque", counts.shortage, counts.shortage > 0 ? "danger" : "neutral"],
                    ["Non comptés", counts.uncounted, counts.uncounted > 0 ? "warn" : "neutral"],
                    ["Références périmées", counts.stale, counts.stale > 0 ? "warn" : "neutral"],
                  ] as const
                ).map(([label, value, tone]) => (
                  <div key={label} className="border-line px-4 py-2.5 [&:not(:last-child)]:border-r max-xl:[&:nth-child(3)]:border-r-0 max-xl:[&:nth-child(-n+3)]:border-b">
                    <dt className="flex items-center gap-1.5 text-meta text-mist">
                      <span className={`size-2 rounded-full ${toneDot[tone]}`} aria-hidden="true" />
                      {label}
                    </dt>
                    <dd className={`mt-0.5 text-[1.25rem] font-semibold tabular-nums ${toneText[tone]}`}>{value}</dd>
                  </div>
                ))}
              </dl>
            </div>
          ) : null}

          {!locked && counts && counts.uncounted > 0 ? (
            <Callout tone="warn" className="mb-3">
              Des produits ne sont pas encore comptés.
            </Callout>
          ) : null}
          {!locked && counts && counts.stale > 0 ? (
            <Callout tone="warn" className="mb-3">
              Le stock de référence a changé pour au moins un produit. Réactualisez la référence ou reprenez le
              comptage avant de valider.
            </Callout>
          ) : null}

          <div className="panel">
            {!locked ? (
              <div className="flex flex-wrap items-center justify-between gap-2 border-b border-line px-4 py-2.5">
                <button type="button" className={`${quietButtonClass} btn-sm`} onClick={() => void openPicker()}>
                  <Icon name="plus" size={14} />
                  Ajouter des produits
                </button>
                <div className="flex items-center gap-3">
                  {detail.lines.length > 1 ? (
                    <span className="text-meta text-faint">Touche Entrée : ligne suivante</span>
                  ) : null}
                  <button
                    type="button"
                    className={`${buttonClass} btn-sm`}
                    disabled={saving || detail.lines.length === 0}
                    onClick={() => void saveCounts()}
                  >
                    {saving ? <Spinner /> : <Icon name="save" size={14} />}
                    Enregistrer les quantités
                  </button>
                </div>
              </div>
            ) : null}
            {detail.lines.length === 0 ? (
              <p className="px-4 py-8 text-center text-table text-mist">Aucun produit dans cet inventaire.</p>
            ) : (
              <div className="table-frame table-frame-flush table-frame-tall rounded-b-panel">
                <table className="data-table min-w-[38rem]">
                  <thead>
                    <tr>
                      <th>Produit / référence</th>
                      <th className="num">Stock de réf.</th>
                      <th className="num">Quantité comptée</th>
                      <th>Écart</th>
                      <th className="col-optional">Saisie</th>
                      <th className="actions">Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {detail.lines.map((line) => (
                      <tr key={line.id} className={line.drifted ? "row-warn" : undefined}>
                        <td className="min-w-[10rem]">
                          <span className="block font-medium">{line.productName}</span>
                          <span className="block text-meta text-faint">
                            <span>{line.productReference ?? "—"}</span>
                            <span aria-hidden="true"> · </span>
                            <span>{line.typeName}</span>
                          </span>
                        </td>
                        <td className="num">{displayQuantity(line.theoretical)}</td>
                        <td className="num">
                          <input
                            aria-label={`Quantité comptée de ${line.productName}`}
                            data-count-line={line.id}
                            value={drafts[line.id] ?? ""}
                            inputMode="decimal"
                            autoComplete="off"
                            disabled={locked || saving}
                            onChange={(event) =>
                              setDrafts((current) => ({ ...current, [line.id]: event.target.value }))
                            }
                            onKeyDown={(event) => {
                              if (event.key === "Enter") {
                                event.preventDefault();
                                focusNextCount(line.id);
                              }
                            }}
                            className="control control-sm control-num ml-auto w-28 font-semibold"
                          />
                          {line.drifted ? (
                            <p className="mt-1 max-w-[14rem] whitespace-normal text-left text-meta text-warn">
                              Le stock théorique est passé de {displayQuantity(line.theoretical)} à{" "}
                              {displayQuantity(line.current)} depuis le comptage.
                            </p>
                          ) : null}
                        </td>
                        <td>
                          <StatusBadge tone={varianceTone(line)}>{varianceText(line)}</StatusBadge>
                        </td>
                        <td className="col-optional">
                          <span
                            className={`inline-flex items-center gap-1 text-meta ${
                              line.counted === null ? "text-warn" : "text-ok"
                            }`}
                          >
                            <Icon name={line.counted === null ? "alert" : "success"} size={13} />
                            <span>{line.counted === null ? "Non compté" : "Compté"}</span>
                          </span>
                        </td>
                        <td className="actions">
                          <div className="ml-auto flex max-w-[15rem] flex-wrap items-center justify-end gap-1">
                            {line.drifted && !locked ? (
                              <>
                                <button
                                  type="button"
                                  className="btn btn-secondary btn-sm"
                                  disabled={saving}
                                  onClick={() => void refresh(line.id, false)}
                                >
                                  Réactualiser la référence
                                </button>
                                <button
                                  type="button"
                                  className="btn btn-secondary btn-sm"
                                  disabled={saving}
                                  onClick={() => void refresh(line.id, true)}
                                >
                                  Reprendre le comptage
                                </button>
                              </>
                            ) : null}
                            <button
                              type="button"
                              className="btn btn-ghost btn-sm px-1.5 text-mist hover:text-gtm-blue"
                              aria-label="Fiche"
                              title="Fiche de stock"
                              onClick={() => onOpenSheet(line.productId)}
                            >
                              <Icon name="sheet" size={16} />
                            </button>
                            {!locked ? (
                              <button
                                type="button"
                                className="btn btn-ghost btn-sm text-mist hover:text-danger"
                                disabled={saving}
                                onClick={() => void remove(line.id)}
                              >
                                Retirer
                              </button>
                            ) : null}
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          {!locked ? (
            <div className="panel mt-4 flex flex-wrap items-center justify-between gap-3 px-4 py-3">
              <div className="min-w-0">
                <p className="font-medium">Validation de l'inventaire</p>
                <p className="text-meta text-mist">
                  {blockers.length > 0
                    ? blockers.join(" ")
                    : "Tous les produits sont comptés. Les écarts non nuls deviendront des corrections de stock."}
                </p>
              </div>
              <button
                type="button"
                className={buttonClass}
                disabled={!canValidate || saving}
                onClick={() => setConfirming(true)}
              >
                <Icon name="check" size={16} />
                Valider l'inventaire
              </button>
            </div>
          ) : null}

          {confirming && !locked ? (
            <Dialog
              title="Valider l'inventaire"
              size="sm"
              onClose={() => setConfirming(false)}
              closeDisabled={saving}
              footer={
                <>
                  <button type="button" className={quietButtonClass} disabled={saving} onClick={() => setConfirming(false)}>
                    Annuler
                  </button>
                  <button
                    type="button"
                    className="btn btn-danger"
                    disabled={saving}
                    data-autofocus
                    onClick={() => void confirmValidation()}
                  >
                    {saving ? <Spinner /> : null}
                    Confirmer la validation
                  </button>
                </>
              }
            >
              <p className="text-body">
                Valider cet inventaire ? Les écarts non nuls créeront des corrections de stock. Cette action est
                définitive.
              </p>
              {counts ? (
                <p className="mt-3 text-table text-mist">
                  {counts.surplus} surplus · {counts.shortage} manque{counts.shortage > 1 ? "s" : ""} ·{" "}
                  {counts.unchanged} sans écart
                </p>
              ) : null}
            </Dialog>
          ) : null}

          {adding && !locked ? (
            <Dialog
              title="Ajouter des produits"
              size="lg"
              onClose={() => setAdding(false)}
              closeDisabled={saving}
              footer={
                <>
                  <p className="mr-auto text-meta text-mist">
                    {selected.length === 0
                      ? "Aucun produit sélectionné."
                      : selected.length === 1
                        ? "1 produit sélectionné."
                        : `${selected.length} produits sélectionnés.`}
                  </p>
                  <button type="button" className={quietButtonClass} disabled={saving} onClick={() => setAdding(false)}>
                    Annuler
                  </button>
                  <button type="button" className={buttonClass} disabled={saving} onClick={() => void addSelected()}>
                    Ajouter la sélection
                  </button>
                </>
              }
            >
              <div className="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-3">
                <FilterField label="Recherche">
                  <input value={search} onChange={(event) => setSearch(event.target.value)} className={controlClass} placeholder="Nom ou référence" />
                </FilterField>
                <FilterField label="Type">
                  <select value={typeId} onChange={(event) => setTypeId(event.target.value)} className={controlClass}>
                    <option value="">Tous les types</option>
                    {types.map((type) => (
                      <option key={type.id} value={type.id}>
                        {type.name}
                      </option>
                    ))}
                  </select>
                </FilterField>
              </div>
              {error ? (
                <Callout tone="danger" role="alert" className="mt-3">
                  {error}
                </Callout>
              ) : null}
              {visibleProducts.length === 0 ? (
                <p className="mt-3 py-6 text-center text-table text-mist">Aucun produit disponible pour ces critères.</p>
              ) : (
                <div className="table-frame mt-3 max-h-80">
                  <table className="data-table">
                    <thead>
                      <tr>
                        <th className="w-10">
                          <span className="sr-only">Sélection</span>
                        </th>
                        <th>Produit</th>
                        <th>Référence</th>
                      </tr>
                    </thead>
                    <tbody>
                      {visibleProducts.map((product) => (
                        <tr key={product.id}>
                          <td>
                            <input
                              id={`pick-${product.id}`}
                              type="checkbox"
                              className="size-4 accent-gtm-blue"
                              checked={selected.includes(product.id)}
                              onChange={(event) => {
                                setSelected((current) =>
                                  event.target.checked
                                    ? [...current, product.id]
                                    : current.filter((id) => id !== product.id),
                                );
                              }}
                            />
                          </td>
                          <td>
                            <label htmlFor={`pick-${product.id}`} className="cursor-pointer font-medium">
                              {product.name}
                            </label>
                          </td>
                          <td className="ref">{product.reference ?? "—"}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </Dialog>
          ) : null}
        </>
      ) : null}
    </section>
  );
}
