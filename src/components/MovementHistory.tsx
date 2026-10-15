import { useEffect, useState } from "react";
import { toErrorMessage } from "../app/api";
import { listProducts } from "../catalogue/api";
import type { Product } from "../catalogue/types";
import { exportHistory } from "../exchange/api";
import { getStockMovement, listStockMovements } from "../stock/api";
import type { StockMovement } from "../stock/api";
import { displayQuantity, movementLabel } from "../stock/quantity";
import { formatDateTime } from "../app/format";
import { ExportBar } from "./ExportBar";
import { controlClass, FilterField } from "./Field";
import { MovementDetailDialog } from "./MovementDetailDialog";
import { Callout, MovementTag } from "./ui/Feedback";
import { Icon } from "./ui/Icon";
import { EmptyState, FilterBar, Loading, PageHeader } from "./ui/Layout";

function directionLabel(movement: Pick<StockMovement, "direction" | "source">) {
  return movementLabel(movement.direction, movement.source);
}

type MovementHistoryProps = {
  onOpenSheet?: (productId: number) => void;
};

export function MovementHistory({ onOpenSheet }: MovementHistoryProps) {
  const [search, setSearch] = useState("");
  const [productId, setProductId] = useState("");
  const [direction, setDirection] = useState<"all" | "in" | "out">("all");
  const [fromDate, setFromDate] = useState("");
  const [toDate, setToDate] = useState("");
  const [products, setProducts] = useState<Product[]>([]);
  const [movements, setMovements] = useState<StockMovement[]>([]);
  const [detail, setDetail] = useState<StockMovement | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    listProducts({ search: "", typeId: "", supplierId: "", status: "all" })
      .then((rows) => {
        if (!cancelled) {
          setProducts(rows);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setProducts([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    listStockMovements({ search, productId, direction })
      .then((rows) => {
        if (!cancelled) {
          setMovements(rows);
          setLoading(false);
        }
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
  }, [search, productId, direction]);

  const filtered = search.trim().length > 0 || productId !== "" || direction !== "all";

  async function openDetail(id: number) {
    setError(null);
    try {
      setDetail(await getStockMovement(id));
    } catch (reason) {
      setError(toErrorMessage(reason));
    }
  }

  return (
    <section>
      <PageHeader
        title="Historique des mouvements"
        description="L'écran affiche au plus 200 mouvements. L'export reprend tous les mouvements des filtres et de la période, sans cette limite."
        actions={
          <ExportBar
            stem="historique-gtm"
            scope={
              filtered || fromDate !== "" || toDate !== ""
                ? "L'export porte sur les filtres et la période, pas seulement sur les 200 lignes affichées."
                : "L'export porte sur tout l'historique, pas seulement sur les 200 lignes affichées."
            }
            beforeSave={() =>
              fromDate !== "" && toDate !== "" && toDate < fromDate
                ? "La date de fin précède la date de début."
                : null
            }
            onExport={(path, format) =>
              exportHistory(path, format, { search, productId, direction, fromDate, toDate })
            }
          />
        }
      />
      <FilterBar className="grid-cols-[minmax(0,1.6fr)_minmax(0,1.2fr)_minmax(0,1.2fr)]">
        <FilterField label="Recherche">
          <span className="relative block">
            <Icon name="search" size={15} className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-faint" />
            <input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              className={`${controlClass} pl-8`}
              placeholder="Produit, référence ou motif"
            />
          </span>
        </FilterField>
        <FilterField label="Produit">
          <select value={productId} onChange={(event) => setProductId(event.target.value)} className={controlClass}>
            <option value="">Tous les produits</option>
            {products.map((product) => (
              <option key={product.id} value={product.id}>
                {product.name}
              </option>
            ))}
          </select>
        </FilterField>
        <FilterField label="Mouvement">
          <select
            value={direction}
            onChange={(event) => setDirection(event.target.value as "all" | "in" | "out")}
            className={controlClass}
          >
            <option value="all">Entrées et sorties</option>
            <option value="in">Entrées</option>
            <option value="out">Sorties</option>
          </select>
        </FilterField>
        <div className="col-span-full flex flex-wrap items-end gap-3 border-t border-line pt-3">
          <span className="mb-2 text-table font-semibold text-ink">Période d'export</span>
          <FilterField label="Du" className="w-40">
            <input type="date" value={fromDate} onChange={(event) => setFromDate(event.target.value)} className={controlClass} />
          </FilterField>
          <FilterField label="Au" className="w-40">
            <input type="date" value={toDate} onChange={(event) => setToDate(event.target.value)} className={controlClass} />
          </FilterField>
          <p className="mb-2 flex items-center gap-1.5 text-meta text-faint">
            <Icon name="info" size={13} />
            La période limite l'export. Elle ne change pas la liste de 200 lignes.
          </p>
        </div>
      </FilterBar>
      {loading ? <Loading>Chargement de l'historique…</Loading> : null}
      {error ? (
        <Callout tone="danger" role="alert" className="mb-4">
          {error}
        </Callout>
      ) : null}
      {!loading && !error && movements.length === 0 ? (
        <EmptyState
          icon="history"
          title={filtered ? "Aucun mouvement ne correspond à ces critères." : "Aucun mouvement n'est enregistré."}
        />
      ) : null}
      {!loading && !error && movements.length > 0 ? (
        <>
          <div className="table-frame table-frame-tall">
            <table className="data-table min-w-[38rem]">
              <thead>
                <tr>
                  <th>Date</th>
                  <th>Produit</th>
                  <th>Mouvement</th>
                  <th className="num">Quantité</th>
                  <th>Motif</th>
                  <th>Réf. document</th>
                  <th className="actions">Actions</th>
                </tr>
              </thead>
              <tbody>
                {movements.map((movement) => (
                  <tr key={movement.id}>
                    <td className="nowrap tabular-nums text-mist">{formatDateTime(movement.createdAt)}</td>
                    <td>
                      {onOpenSheet ? (
                        <button type="button" className="link" onClick={() => onOpenSheet(movement.productId)}>
                          {movement.productName}
                        </button>
                      ) : (
                        <span className="font-medium">{movement.productName}</span>
                      )}
                    </td>
                    <td>
                      <MovementTag direction={movement.direction} label={directionLabel(movement)} />
                    </td>
                    <td className="num font-semibold">{displayQuantity(movement.quantity)}</td>
                    <td className="max-w-[12rem] truncate" title={movement.reason ?? undefined}>
                      {movement.reason ?? <span className="text-faint">—</span>}
                    </td>
                    <td className="ref">{movement.documentReference ?? "—"}</td>
                    <td className="actions">
                      <button type="button" className="btn btn-ghost btn-sm" onClick={() => void openDetail(movement.id)}>
                        Détail
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="mt-2 text-meta text-faint">
            {movements.length === 1 ? "1 mouvement affiché" : `${movements.length} mouvements affichés`}
            {movements.length >= 200 ? " — limite d'affichage atteinte, affinez les filtres ou exportez." : "."}
          </p>
        </>
      ) : null}
      {detail ? <MovementDetailDialog movement={detail} onClose={() => setDetail(null)} /> : null}
    </section>
  );
}
