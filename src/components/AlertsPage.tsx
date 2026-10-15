import { useEffect, useState } from "react";
import { toErrorMessage } from "../app/api";
import { listProductTypes, listSuppliers } from "../catalogue/api";
import type { NamedRecord } from "../catalogue/types";
import { listReorderAlerts, setReorderThreshold } from "../inventory/api";
import type { ReorderAlert } from "../inventory/api";
import { displayQuantity, parseNonNegative } from "../stock/quantity";
import { exportReorderAlerts } from "../exchange/api";
import { ExportBar } from "./ExportBar";
import { controlClass, FilterField } from "./Field";
import { StatusBadge } from "./StatusBadge";
import { Callout } from "./ui/Feedback";
import { Icon } from "./ui/Icon";
import { EmptyState, FilterBar, Loading, PageHeader } from "./ui/Layout";

type AlertsPageProps = {
  onOpenSheet: (productId: number) => void;
  onOpenEntry: (productId: number) => void;
  onStockChanged: () => void;
};

function gapText(gap: string) {
  if (gap === "0") {
    return "Au seuil";
  }
  const amount = gap.startsWith("-") ? gap.slice(1) : gap;
  return `Sous le seuil de ${displayQuantity(amount)}`;
}

export function AlertsPage({ onOpenSheet, onOpenEntry, onStockChanged }: AlertsPageProps) {
  const [alerts, setAlerts] = useState<ReorderAlert[]>([]);
  const [types, setTypes] = useState<NamedRecord[]>([]);
  const [suppliers, setSuppliers] = useState<NamedRecord[]>([]);
  const [search, setSearch] = useState("");
  const [typeId, setTypeId] = useState("");
  const [supplierId, setSupplierId] = useState("");
  const [sort, setSort] = useState<"asc" | "desc">("asc");
  const [drafts, setDrafts] = useState<Record<number, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    Promise.all([
      listReorderAlerts({ search, typeId, supplierId, sort }),
      listProductTypes(false),
      listSuppliers(false),
    ])
      .then(([nextAlerts, nextTypes, nextSuppliers]) => {
        if (cancelled) {
          return;
        }
        setAlerts(nextAlerts);
        setTypes(nextTypes);
        setSuppliers(nextSuppliers);
        setDrafts(
          Object.fromEntries(nextAlerts.map((alert) => [alert.productId, displayQuantity(alert.threshold)])),
        );
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
  }, [search, typeId, supplierId, sort, revision]);

  async function saveThreshold(productId: number, name: string) {
    const raw = (drafts[productId] ?? "").trim();
    const parsed = parseNonNegative(raw);
    if ("error" in parsed) {
      setError(`${name} : ${parsed.error.replace("quantité", "seuil").replace("La quantité", "Le seuil")}`);
      return;
    }
    setError(null);
    try {
      await setReorderThreshold(productId, raw.replace(",", "."));
      setRevision((current) => current + 1);
      onStockChanged();
    } catch (reason) {
      setError(toErrorMessage(reason));
    }
  }

  async function disable(productId: number) {
    setError(null);
    try {
      await setReorderThreshold(productId, null);
      setRevision((current) => current + 1);
      onStockChanged();
    } catch (reason) {
      setError(toErrorMessage(reason));
    }
  }

  const filtered = search.trim() !== "" || typeId !== "" || supplierId !== "";

  const ruptures = alerts.filter((alert) => alert.quantity === "0").length;

  return (
    <section>
      <PageHeader
        title="Réapprovisionnement"
        description="Un produit actif est en alerte lorsque son stock disponible est inférieur ou égal à son seuil. Le statut est recalculé à partir du stock courant."
        actions={
          <ExportBar
            stem="reapprovisionnement-gtm"
            scope={
              filtered
                ? "L'export reprend les alertes des filtres affichés, calculées sur le stock courant."
                : "L'export reprend les alertes affichées, calculées sur le stock courant."
            }
            onExport={(path, format) => exportReorderAlerts(path, format, { search, typeId, supplierId, sort })}
          />
        }
      />
      <FilterBar className="grid-cols-[minmax(0,1.6fr)_repeat(3,minmax(0,1fr))]">
        <FilterField label="Recherche">
          <span className="relative block">
            <Icon name="search" size={15} className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-faint" />
            <input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              className={`${controlClass} pl-8`}
              placeholder="Nom ou référence"
            />
          </span>
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
        <FilterField label="Fournisseur">
          <select value={supplierId} onChange={(event) => setSupplierId(event.target.value)} className={controlClass}>
            <option value="">Tous les fournisseurs</option>
            {suppliers.map((supplier) => (
              <option key={supplier.id} value={supplier.id}>
                {supplier.name}
              </option>
            ))}
          </select>
        </FilterField>
        <FilterField label="Tri">
          <select
            aria-label="Tri"
            value={sort}
            onChange={(event) => setSort(event.target.value as "asc" | "desc")}
            className={controlClass}
          >
            <option value="asc">Stock croissant</option>
            <option value="desc">Stock décroissant</option>
          </select>
        </FilterField>
      </FilterBar>
      {error ? (
        <Callout tone="danger" role="alert" className="mb-4">
          {error}
        </Callout>
      ) : null}
      {loading ? <Loading>Chargement des alertes…</Loading> : null}
      {!loading && alerts.length === 0 ? (
        <EmptyState icon="success" title={filtered ? "Aucun produit à réapprovisionner ne correspond à ces critères." : "Aucun produit à réapprovisionner."}>
          {filtered ? "Modifiez la recherche ou les filtres." : "Tous les produits suivis sont au-dessus de leur seuil."}
        </EmptyState>
      ) : null}
      {alerts.length > 0 ? (
        <>
          <div className="table-frame table-frame-tall">
            <table className="data-table min-w-[38rem]">
              <thead>
                <tr>
                  <th>Produit / référence</th>
                  <th>Fournisseur</th>
                  <th className="num">Stock</th>
                  <th>Seuil</th>
                  <th>Écart</th>
                  <th className="actions">Action</th>
                </tr>
              </thead>
              <tbody>
                {alerts.map((alert) => {
                  const rupture = alert.quantity === "0";
                  return (
                    <tr key={alert.productId} className={rupture ? "row-danger" : "row-warn"}>
                      <td className="min-w-[10rem]">
                        <span className="block font-medium">{alert.name}</span>
                        <span className="block text-meta text-faint">
                          <span>{alert.reference ?? "—"}</span>
                          <span aria-hidden="true"> · </span>
                          <span>{alert.typeName}</span>
                        </span>
                      </td>
                      <td>{alert.supplierName ?? <span className="text-faint">—</span>}</td>
                      <td className="num">
                        <span className="inline-flex items-center gap-2">
                          {rupture ? (
                            <StatusBadge tone="danger" dot={false}>
                              Rupture
                            </StatusBadge>
                          ) : null}
                          <span className={`text-section font-semibold ${rupture ? "text-danger" : "text-ink"}`}>
                            {displayQuantity(alert.quantity)}
                          </span>
                        </span>
                      </td>
                      <td>
                        <div className="flex items-center gap-1">
                          <input
                            aria-label={`Seuil de ${alert.name}`}
                            value={drafts[alert.productId] ?? ""}
                            inputMode="decimal"
                            onChange={(event) =>
                              setDrafts((current) => ({ ...current, [alert.productId]: event.target.value }))
                            }
                            onKeyDown={(event) => {
                              if (event.key === "Enter") {
                                event.preventDefault();
                                void saveThreshold(alert.productId, alert.name);
                              }
                            }}
                            className="control control-sm control-num w-[4.5rem]"
                          />
                          <button
                            type="button"
                            className="btn btn-ghost btn-sm px-1.5"
                            aria-label={`Modifier le seuil de ${alert.name}`}
                            title="Enregistrer ce seuil"
                            onClick={() => void saveThreshold(alert.productId, alert.name)}
                          >
                            <Icon name="check" size={16} />
                          </button>
                          <button
                            type="button"
                            className="btn btn-ghost btn-sm px-1.5 text-mist hover:bg-danger-bg! hover:text-danger"
                            aria-label={`Désactiver le seuil de ${alert.name}`}
                            title="Désactiver le seuil (retire le produit des alertes)"
                            onClick={() => void disable(alert.productId)}
                          >
                            <Icon name="ban" size={15} />
                          </button>
                        </div>
                      </td>
                      <td>
                        <StatusBadge tone={alert.gap === "0" ? "warn" : "danger"}>{gapText(alert.gap)}</StatusBadge>
                      </td>
                      <td className="actions">
                        <div className="flex items-center justify-end gap-1">
                          <button
                            type="button"
                            className="btn btn-ghost btn-sm px-1.5 text-mist hover:text-gtm-blue"
                            aria-label="Fiche"
                            title="Fiche de stock"
                            onClick={() => onOpenSheet(alert.productId)}
                          >
                            <Icon name="sheet" size={16} />
                          </button>
                          <button
                            type="button"
                            className="btn btn-primary btn-sm"
                            aria-label="Entrée de stock"
                            title="Enregistrer une entrée de stock pour ce produit"
                            onClick={() => onOpenEntry(alert.productId)}
                          >
                            <Icon name="entry" size={14} />
                            <span className="btn-label">Entrée</span>
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          <p className="mt-2 text-meta text-faint">
            {alerts.length === 1 ? "1 produit à réapprovisionner" : `${alerts.length} produits à réapprovisionner`}
            {ruptures > 0 ? `, dont ${ruptures} en rupture.` : "."}
          </p>
        </>
      ) : null}
    </section>
  );
}
