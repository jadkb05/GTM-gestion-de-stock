import { useEffect, useRef, useState, type FormEvent } from "react";
import { toErrorMessage } from "../app/api";
import { listProducts } from "../catalogue/api";
import type { Product } from "../catalogue/types";
import { getProductStock, recordStockEntry, recordStockExit } from "../stock/api";
import { displayQuantity, formatQuantity, optionalText, parseNonNegative, parseQuantity } from "../stock/quantity";
import { buttonClass, controlClass, Field, quietButtonClass, RequiredLegend } from "./Field";
import { Callout, Spinner } from "./ui/Feedback";
import { Icon } from "./ui/Icon";
import { Detail, Loading, PageHeader } from "./ui/Layout";

type MovementFormProps = {
  direction: "in" | "out";
  initialProductId: number | null;
  onRecorded?: () => void;
};

const emptyFields = {
  quantity: "",
  reason: "",
  documentReference: "",
  comment: "",
};

/* Filtre la liste par nom ou référence, sans jamais masquer le produit choisi. */
function filterProducts(products: Product[], filter: string, selectedId: string): Product[] {
  const needle = filter.trim().toLowerCase();
  if (!needle) {
    return products;
  }
  return products.filter(
    (product) =>
      String(product.id) === selectedId ||
      product.name.toLowerCase().includes(needle) ||
      (product.reference ?? "").toLowerCase().includes(needle),
  );
}

export function MovementForm({ direction, initialProductId, onRecorded }: MovementFormProps) {
  const entry = direction === "in";
  const [products, setProducts] = useState<Product[]>([]);
  const [productId, setProductId] = useState(initialProductId ? String(initialProductId) : "");
  const [productFilter, setProductFilter] = useState("");
  const [fields, setFields] = useState(emptyFields);
  const [stock, setStock] = useState<string | null>(null);
  const [inactive, setInactive] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [fieldError, setFieldError] = useState<string | null>(null);
  const [review, setReview] = useState(false);
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  const [notice, setNotice] = useState<string | null>(null);
  const recapRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (review) {
      recapRef.current?.scrollIntoView?.({ block: "nearest" });
    }
  }, [review]);

  useEffect(() => {
    let cancelled = false;
    listProducts({ search: "", typeId: "", supplierId: "", status: "active" })
      .then((rows) => {
        if (!cancelled) {
          setProducts(rows);
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
  }, []);

  useEffect(() => {
    if (initialProductId) {
      setProductId(String(initialProductId));
    }
  }, [initialProductId]);

  useEffect(() => {
    if (productId.trim().length === 0) {
      setStock(null);
      setInactive(false);
      return;
    }
    let cancelled = false;
    getProductStock(Number(productId))
      .then((level) => {
        if (cancelled) {
          return;
        }
        setStock(level.quantity);
        setInactive(!level.isActive);
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setStock(null);
          setFieldError(toErrorMessage(reason));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [productId]);

  function update(field: keyof typeof emptyFields, value: string) {
    setFields((current) => ({ ...current, [field]: value }));
    setReview(false);
    setNotice(null);
    setFieldError(null);
  }

  function validationError(): string | null {
    if (productId.trim().length === 0) {
      return "Choisissez un produit.";
    }
    if (inactive) {
      return "Ce produit est désactivé.";
    }
    const parsed = parseQuantity(fields.quantity);
    if ("error" in parsed) {
      return parsed.error;
    }
    const reason = optionalText(fields.reason, 200, "Le motif dépasse 200 caractères.");
    if (reason) {
      return reason;
    }
    const document = optionalText(
      fields.documentReference,
      64,
      "La référence documentaire dépasse 64 caractères.",
    );
    if (document) {
      return document;
    }
    const comment = optionalText(fields.comment, 2000, "Le commentaire dépasse 2 000 caractères.");
    if (comment) {
      return comment;
    }
    if (stock === null) {
      return "Le stock de ce produit n'est pas encore connu.";
    }
    if (!entry) {
      const available = parseNonNegative(stock);
      if ("error" in available || parsed.milli > available.milli) {
        return "Le stock disponible est insuffisant.";
      }
    }
    return null;
  }

  function verify(event: FormEvent) {
    event.preventDefault();
    const message = validationError();
    setFieldError(message);
    setReview(message === null);
  }

  async function submit() {
    if (savingRef.current || validationError()) {
      return;
    }
    savingRef.current = true;
    setSaving(true);
    setNotice(null);
    try {
      const input = { productId, ...fields };
      const saved = entry ? await recordStockEntry(input) : await recordStockExit(input);
      const level = await getProductStock(saved.productId);
      setStock(level.quantity);
      setFields(emptyFields);
      setReview(false);
      setNotice(
        entry
          ? `Entrée enregistrée. Stock disponible : ${displayQuantity(level.quantity)}.`
          : `Sortie enregistrée. Stock disponible : ${displayQuantity(level.quantity)}.`,
      );
      onRecorded?.();
    } catch (reason) {
      setFieldError(toErrorMessage(reason));
      setReview(false);
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  }

  const parsed = parseQuantity(fields.quantity);
  const available = stock === null ? null : parseNonNegative(stock);
  const after =
    "milli" in parsed && available && "milli" in available
      ? formatQuantity(entry ? available.milli + parsed.milli : available.milli - parsed.milli)
      : null;
  const visibleProducts = filterProducts(products, productFilter, productId);
  const selected = products.find((product) => String(product.id) === productId);

  const negative = after !== null && after.startsWith("-");
  const insufficient =
    !entry && stock !== null && "milli" in parsed && available !== null && "milli" in available && parsed.milli > available.milli;
  const operation = entry ? "Entrée" : "Sortie";

  return (
    <section>
      <PageHeader
        title={entry ? "Entrées de stock" : "Sorties de stock"}
        description="La quantité est un nombre positif, avec au plus trois décimales. Aucune unité n'est imposée."
      />
      {loading ? <Loading>Chargement des produits…</Loading> : null}
      {error ? (
        <Callout tone="danger" role="alert">
          {error}
        </Callout>
      ) : null}
      {!loading && !error ? (
        <form onSubmit={verify} noValidate className="grid items-start gap-5 lg:grid-cols-[minmax(0,1fr)_19rem]">
          <div className="min-w-0 space-y-5">
            <div className="panel">
              <div className="panel-header">
                <h2 className="panel-title flex items-center gap-2">
                  <span
                    className={`flex size-6 items-center justify-center rounded-control ${
                      entry ? "bg-ok-bg text-ok" : "bg-warn-bg text-[#7a3d00]"
                    }`}
                  >
                    <Icon name={entry ? "entry" : "exit"} size={14} />
                  </span>
                  {entry ? "Nouvelle entrée" : "Nouvelle sortie"}
                </h2>
                <RequiredLegend />
              </div>
              <div className="grid gap-4 p-4 sm:grid-cols-2">
                <div className="sm:col-span-2">
                  <label htmlFor="movement-product-filter" className="field-label">
                    Rechercher un produit
                  </label>
                  <span className="relative block">
                    <Icon
                      name="search"
                      size={15}
                      className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-faint"
                    />
                    <input
                      id="movement-product-filter"
                      value={productFilter}
                      autoComplete="off"
                      placeholder="Nom ou référence, puis choisissez dans la liste"
                      onChange={(event) => {
                        const next = event.target.value;
                        setProductFilter(next);
                        const matches = filterProducts(products, next, "");
                        if (matches.length === 1 && String(matches[0].id) !== productId) {
                          setProductId(String(matches[0].id));
                          setReview(false);
                          setNotice(null);
                          setFieldError(null);
                        }
                      }}
                      className={`${controlClass} pl-8`}
                    />
                  </span>
                  {productFilter.trim() !== "" ? (
                    <p className="mt-1 text-meta text-faint">
                      {visibleProducts.length === 0
                        ? "Aucun produit actif ne correspond."
                        : visibleProducts.length === 1
                          ? "1 produit correspond."
                          : `${visibleProducts.length} produits correspondent.`}
                    </p>
                  ) : null}
                </div>
                <Field id="movement-product" label="Produit" required className="sm:col-span-2">
                  <select
                    id="movement-product"
                    value={productId}
                    onChange={(event) => {
                      setProductId(event.target.value);
                      setReview(false);
                      setNotice(null);
                      setFieldError(null);
                    }}
                    className={controlClass}
                  >
                    <option value="">Choisir un produit actif</option>
                    {visibleProducts.map((product) => (
                      <option key={product.id} value={product.id}>
                        {product.name}
                        {product.reference ? ` — ${product.reference}` : ""}
                      </option>
                    ))}
                  </select>
                </Field>
                {selected ? (
                  <p className="-mt-2 text-meta text-faint sm:col-span-2">
                    {selected.typeName}
                    {selected.supplierName ? ` · ${selected.supplierName}` : ""}
                  </p>
                ) : null}
                {inactive ? (
                  <Callout tone="danger" role="alert" className="sm:col-span-2">
                    Ce produit est désactivé. Son stock reste consultable, sans nouvelle opération.
                  </Callout>
                ) : null}
                <Field id="movement-quantity" label="Quantité" required>
                  <input
                    id="movement-quantity"
                    value={fields.quantity}
                    inputMode="decimal"
                    autoComplete="off"
                    aria-invalid={insufficient}
                    onChange={(event) => update("quantity", event.target.value)}
                    className={`${controlClass} control-num font-semibold`}
                  />
                </Field>
                <Field id="movement-document" label="Référence documentaire" hint="Bon de livraison, bon de sortie…">
                  <input
                    id="movement-document"
                    value={fields.documentReference}
                    onChange={(event) => update("documentReference", event.target.value)}
                    className={controlClass}
                  />
                </Field>
                <Field id="movement-reason" label="Motif" className="sm:col-span-2">
                  <input
                    id="movement-reason"
                    value={fields.reason}
                    onChange={(event) => update("reason", event.target.value)}
                    className={controlClass}
                  />
                </Field>
                <Field id="movement-comment" label="Commentaire" className="sm:col-span-2">
                  <textarea
                    id="movement-comment"
                    rows={3}
                    value={fields.comment}
                    onChange={(event) => update("comment", event.target.value)}
                    className={controlClass}
                  />
                </Field>
                {fieldError ? (
                  <Callout tone="danger" role="alert" className="sm:col-span-2">
                    {fieldError}
                  </Callout>
                ) : null}
                {notice ? (
                  <Callout tone="ok" role="status" className="sm:col-span-2">
                    {notice}
                  </Callout>
                ) : null}
              </div>
              {!review ? (
                <div className="flex justify-end rounded-b-panel border-t border-line bg-surface-muted px-4 py-3">
                  <button type="submit" className={buttonClass} disabled={inactive}>
                    Vérifier l'opération
                    <Icon name="forward" size={16} />
                  </button>
                </div>
              ) : null}
            </div>

            {review && selected && after ? (
              <div ref={recapRef} className="panel border-gtm-blue-line shadow-[inset_3px_0_0_var(--color-gtm-blue)]">
                <div className="panel-header">
                  <h2 className="panel-title">Récapitulatif</h2>
                  <span className="text-meta text-mist">Vérifiez avant d'enregistrer.</span>
                </div>
                <dl className="grid grid-cols-2 gap-x-6 gap-y-3 p-4 text-table sm:grid-cols-4">
                  <Detail label="Produit" className="col-span-2 sm:col-span-4">
                    {selected.name}
                  </Detail>
                  <Detail label="Opération">{operation}</Detail>
                  <Detail label="Quantité">
                    <span className="tabular-nums">{displayQuantity(formatQuantity("milli" in parsed ? parsed.milli : 0))}</span>
                  </Detail>
                  <Detail label="Stock actuel">
                    <span className="tabular-nums">{stock === null ? "—" : displayQuantity(stock)}</span>
                  </Detail>
                  <Detail label="Stock après opération">
                    <span className="tabular-nums">{displayQuantity(after)}</span>
                  </Detail>
                </dl>
                <div className="flex justify-end gap-2 rounded-b-panel border-t border-line bg-surface-muted px-4 py-3">
                  <button type="button" className={quietButtonClass} disabled={saving} onClick={() => setReview(false)}>
                    Corriger
                  </button>
                  <button type="button" className={buttonClass} disabled={saving} onClick={() => void submit()}>
                    {saving ? <Spinner /> : <Icon name="check" size={16} />}
                    {saving ? "Enregistrement…" : entry ? "Enregistrer l'entrée" : "Enregistrer la sortie"}
                  </button>
                </div>
              </div>
            ) : null}
          </div>

          <aside className="panel lg:sticky lg:top-0" aria-label="Situation du stock">
            <div className="panel-header">
              <h2 className="panel-title">Situation du stock</h2>
            </div>
            <dl className="divide-y divide-line">
              <div className="flex items-baseline justify-between gap-3 px-4 py-3">
                <dt className="text-table text-mist">Stock actuel</dt>
                <dd className="text-[1.125rem] font-semibold tabular-nums">{stock === null ? "—" : displayQuantity(stock)}</dd>
              </div>
              <div className="flex items-baseline justify-between gap-3 px-4 py-3">
                <dt className="text-table text-mist">Quantité saisie</dt>
                <dd className={`text-[1.125rem] font-semibold tabular-nums ${entry ? "text-ok" : "text-[#7a3d00]"}`}>
                  {"milli" in parsed ? `${entry ? "+" : "−"} ${displayQuantity(formatQuantity(parsed.milli))}` : "—"}
                </dd>
              </div>
              <div className={`flex items-baseline justify-between gap-3 px-4 py-3 ${negative ? "bg-danger-bg" : "bg-surface-muted"}`}>
                <dt className="text-table font-medium text-ink">Stock projeté</dt>
                <dd className={`text-figure font-semibold tabular-nums ${negative ? "text-danger" : "text-ink"}`}>
                  {after === null ? "—" : displayQuantity(after)}
                </dd>
              </div>
            </dl>
            {insufficient ? (
              <div className="border-t border-danger-line p-3">
                <Callout tone="danger" role="alert">
                  Le stock disponible est insuffisant.
                </Callout>
              </div>
            ) : (
              <p className="border-t border-line px-4 py-2.5 text-meta text-faint">
                {entry
                  ? "Le stock projeté tient compte de la quantité saisie."
                  : "Une sortie ne peut pas rendre le stock négatif."}
              </p>
            )}
          </aside>
        </form>
      ) : null}
    </section>
  );
}
