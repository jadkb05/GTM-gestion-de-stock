import { useEffect, useState } from "react";
import { toErrorMessage } from "../app/api";
import { plural } from "../app/format";
import {
  listProductTypes,
  listProducts,
  listSuppliers,
  setProductActive,
} from "../catalogue/api";
import type { NamedRecord, Product, ProductStatusFilter } from "../catalogue/types";
import { listStockLevels } from "../stock/api";
import { displayQuantity } from "../stock/quantity";
import { exportCatalogue } from "../exchange/api";
import { buttonClass, controlClass, FilterField, quietButtonClass } from "./Field";
import { ExportBar } from "./ExportBar";
import { IMPORT_COLUMNS_HINT, ImportArticles } from "./ImportArticles";
import { StatusBadge } from "./StatusBadge";
import { ProductForm } from "./ProductForm";
import { Callout } from "./ui/Feedback";
import { Icon } from "./ui/Icon";
import { EmptyState, FilterBar, Loading, PageHeader } from "./ui/Layout";

type Editor =
  | { mode: "closed" }
  | { mode: "create" }
  | { mode: "edit"; product: Product }
  | { mode: "saved"; product: Product };

type ProductCatalogueProps = {
  onEntry?: (productId: number) => void;
  onExit?: (productId: number) => void;
  onOpenSheet?: (productId: number) => void;
};

const iconButton = "btn btn-ghost btn-sm px-1.5 text-mist hover:text-gtm-blue";

export function ProductCatalogue({ onEntry, onExit, onOpenSheet }: ProductCatalogueProps) {
  const [search, setSearch] = useState("");
  const [typeId, setTypeId] = useState("");
  const [supplierId, setSupplierId] = useState("");
  const [status, setStatus] = useState<ProductStatusFilter>("active");
  const [types, setTypes] = useState<NamedRecord[]>([]);
  const [suppliers, setSuppliers] = useState<NamedRecord[]>([]);
  const [products, setProducts] = useState<Product[]>([]);
  const [stock, setStock] = useState<Map<number, string>>(new Map());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const [editor, setEditor] = useState<Editor>({ mode: "closed" });
  const [pendingDisableId, setPendingDisableId] = useState<number | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    Promise.all([
      listProductTypes(true),
      listSuppliers(true),
      listProducts({ search, typeId, supplierId, status }),
      listStockLevels(),
    ])
      .then(([nextTypes, nextSuppliers, nextProducts, levels]) => {
        if (cancelled) {
          return;
        }
        setTypes(nextTypes);
        setSuppliers(nextSuppliers);
        setProducts(nextProducts);
        setStock(new Map(levels.map((level) => [level.productId, level.quantity])));
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (cancelled) {
          return;
        }
        setError(toErrorMessage(reason));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [search, typeId, supplierId, status, reloadToken]);

  const filtered =
    search.trim().length > 0 || typeId !== "" || supplierId !== "" || status !== "active";

  async function confirmDisable(product: Product) {
    setActionError(null);
    try {
      await setProductActive(product.id, false);
      setPendingDisableId(null);
      setReloadToken((value) => value + 1);
    } catch (reason) {
      setActionError(toErrorMessage(reason));
    }
  }

  if (editor.mode === "create" || editor.mode === "edit") {
    return (
      <section>
        <PageHeader
          title="Catalogue des produits"
          back={{ label: "Retour à la liste", onClick: () => setEditor({ mode: "closed" }) }}
        />
        <ProductForm
          types={types}
          suppliers={suppliers}
          product={editor.mode === "edit" ? editor.product : null}
          onCancel={() => setEditor({ mode: "closed" })}
          onSaved={(product) => {
            setEditor({ mode: "saved", product });
            setReloadToken((value) => value + 1);
          }}
        />
      </section>
    );
  }

  const ruptures = products.filter((product) => product.isActive && stock.get(product.id) === "0").length;

  return (
    <section>
      <PageHeader
        title="Catalogue des produits"
        description="Références, types, fournisseurs et stock disponible de chaque produit."
        actions={
          <>
            <ImportArticles onImported={() => setReloadToken((value) => value + 1)} />
            <ExportBar
              stem="catalogue-gtm"
              scope={
                filtered
                  ? "L'export reprend les produits correspondant aux filtres affichés, avec le stock du moment."
                  : "L'export reprend les produits actifs affichés, avec le stock du moment."
              }
              onExport={(path, format) => exportCatalogue(path, format, { search, typeId, supplierId, status })}
            />
            <button type="button" className={buttonClass} onClick={() => setEditor({ mode: "create" })}>
              <Icon name="plus" size={16} />
              Nouveau produit
            </button>
          </>
        }
      />

      {editor.mode === "saved" ? (
        <Callout tone="ok" role="status" className="mb-4">
          Produit enregistré : {editor.product.name}.
        </Callout>
      ) : null}

      <FilterBar className="grid-cols-[minmax(0,1.5fr)_repeat(3,minmax(0,1fr))]">
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
          <select
            value={supplierId}
            onChange={(event) => setSupplierId(event.target.value)}
            className={controlClass}
          >
            <option value="">Tous les fournisseurs</option>
            {suppliers.map((supplier) => (
              <option key={supplier.id} value={supplier.id}>
                {supplier.name}
              </option>
            ))}
          </select>
        </FilterField>
        <FilterField label="Statut">
          <select
            value={status}
            onChange={(event) => setStatus(event.target.value as ProductStatusFilter)}
            className={controlClass}
          >
            <option value="active">Actifs</option>
            <option value="inactive">Désactivés</option>
            <option value="all">Tous</option>
          </select>
        </FilterField>
      </FilterBar>

      {loading ? <Loading>Chargement du catalogue…</Loading> : null}
      {error ? (
        <Callout
          tone="danger"
          role="alert"
          className="mb-4"
          action={
            <button type="button" className={`${quietButtonClass} btn-sm`} onClick={() => setReloadToken((value) => value + 1)}>
              Réessayer
            </button>
          }
        >
          {error}
        </Callout>
      ) : null}
      {actionError ? (
        <Callout tone="danger" role="alert" className="mb-4">
          {actionError}
        </Callout>
      ) : null}

      {!loading && !error && products.length === 0 ? (
        <EmptyState
          title={filtered ? "Aucun produit ne correspond à ces critères." : "Aucun produit n'est enregistré."}
        >
          {filtered ? (
            "Modifiez la recherche ou les filtres."
          ) : (
            <>
              Créez un produit avec « Nouveau produit » ou importez votre liste d'articles avec « Importer ».
              <span className="mt-1 block text-meta">{IMPORT_COLUMNS_HINT}</span>
            </>
          )}
        </EmptyState>
      ) : null}

      {!loading && !error && products.length > 0 ? (
        <>
          <div className="table-frame table-frame-tall">
            <table className="data-table min-w-[38rem]">
              <thead>
                <tr>
                  <th>Produit / référence</th>
                  <th className="col-optional">Type</th>
                  <th>Fournisseur</th>
                  <th className="num">Stock</th>
                  <th>Statut</th>
                  <th className="actions">Actions</th>
                </tr>
              </thead>
              <tbody>
                {products.map((product) => {
                  const known = stock.has(product.id);
                  const quantity = stock.get(product.id) ?? "0";
                  const rupture = product.isActive && known && quantity === "0";
                  return (
                    <tr
                      key={product.id}
                      className={rupture ? "row-danger" : product.isActive ? undefined : "row-muted"}
                    >
                      <td className="min-w-[10rem]">
                        <span className={`block font-medium ${product.isActive ? "text-ink" : "text-mist"}`}>{product.name}</span>
                        <span className="block text-meta text-faint">{product.reference ?? "—"}</span>
                      </td>
                      <td className="col-optional">{product.typeName}</td>
                      <td>{product.supplierName ?? "—"}</td>
                      <td className="num">
                        {known ? (
                          <span className="inline-flex items-center gap-2">
                            {rupture ? <StatusBadge tone="danger">Rupture</StatusBadge> : null}
                            <span className={`font-semibold ${rupture ? "text-danger" : ""}`}>
                              {displayQuantity(quantity)}
                            </span>
                          </span>
                        ) : (
                          <span className="text-faint">Indisponible</span>
                        )}
                      </td>
                      <td>
                        <StatusBadge tone={product.isActive ? "ok" : "neutral"}>
                          {product.isActive ? "Actif" : "Désactivé"}
                        </StatusBadge>
                      </td>
                      <td className="actions">
                        {pendingDisableId === product.id ? (
                          <div className="flex items-center justify-end gap-1.5">
                            <span className="mr-0.5 text-meta font-medium text-danger">Désactiver ?</span>
                            <button type="button" className="btn btn-danger btn-sm" autoFocus onClick={() => confirmDisable(product)}>
                              Confirmer
                            </button>
                            <button
                              type="button"
                              className="btn btn-secondary btn-sm"
                              onClick={() => setPendingDisableId(null)}
                            >
                              Annuler
                            </button>
                          </div>
                        ) : (
                          <div className="flex items-center justify-end gap-1">
                            {product.isActive && onEntry ? (
                              <button type="button" className="btn btn-secondary btn-sm" onClick={() => onEntry(product.id)}>
                                <Icon name="entry" size={14} className="text-ok" />
                                <span className="btn-label">Entrée</span>
                              </button>
                            ) : null}
                            {product.isActive && onExit ? (
                              <button type="button" className="btn btn-secondary btn-sm" onClick={() => onExit(product.id)}>
                                <Icon name="exit" size={14} className="text-[#7a3d00]" />
                                <span className="btn-label">Sortie</span>
                              </button>
                            ) : null}
                            <span className="mx-1 h-5 w-px bg-line" aria-hidden="true" />
                            {onOpenSheet ? (
                              <button
                                type="button"
                                className={iconButton}
                                aria-label="Fiche"
                                title="Fiche de stock"
                                onClick={() => onOpenSheet(product.id)}
                              >
                                <Icon name="sheet" size={16} />
                              </button>
                            ) : null}
                            <button
                              type="button"
                              className={iconButton}
                              aria-label="Modifier"
                              title="Modifier le produit"
                              onClick={() => setEditor({ mode: "edit", product })}
                            >
                              <Icon name="pencil" size={15} />
                            </button>
                            {product.isActive ? (
                              <button
                                type="button"
                                className="btn btn-ghost btn-sm px-1.5 text-mist hover:bg-danger-bg! hover:text-danger"
                                aria-label="Désactiver"
                                title="Désactiver le produit"
                                onClick={() => setPendingDisableId(product.id)}
                              >
                                <Icon name="ban" size={15} />
                              </button>
                            ) : (
                              <button
                                type="button"
                                className="btn btn-ghost btn-sm"
                                onClick={() => {
                                  void setProductActive(product.id, true)
                                    .then(() => setReloadToken((value) => value + 1))
                                    .catch((reason: unknown) => setActionError(toErrorMessage(reason)));
                                }}
                              >
                                Réactiver
                              </button>
                            )}
                          </div>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          <p className="mt-2 text-meta text-faint">
            {plural(products.length, "produit affiché", "produits affichés")}
            {ruptures > 0 ? ` · ${plural(ruptures, "rupture de stock", "ruptures de stock")}` : ""}
          </p>
        </>
      ) : null}
    </section>
  );
}
