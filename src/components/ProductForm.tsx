import { useState, type FormEvent } from "react";
import { createProduct, updateProduct } from "../catalogue/api";
import type { NamedRecord, Product, ProductInput } from "../catalogue/types";
import { validateProduct } from "../catalogue/validation";
import { toErrorMessage } from "../app/api";
import { buttonClass, controlClass, Field, quietButtonClass, RequiredLegend } from "./Field";
import { Callout, Spinner } from "./ui/Feedback";

type ProductFormProps = {
  types: NamedRecord[];
  suppliers: NamedRecord[];
  product: Product | null;
  onCancel: () => void;
  onSaved: (product: Product) => void;
};

function initialInput(product: Product | null): ProductInput {
  if (!product) {
    return { name: "", reference: "", typeId: "", supplierId: "", description: "" };
  }
  return {
    name: product.name,
    reference: product.reference ?? "",
    typeId: String(product.typeId),
    supplierId: product.supplierId === null ? "" : String(product.supplierId),
    description: product.description ?? "",
  };
}

function options(records: NamedRecord[], currentId: string) {
  return records.filter((record) => record.isActive || String(record.id) === currentId);
}

export function ProductForm({ types, suppliers, product, onCancel, onSaved }: ProductFormProps) {
  const [input, setInput] = useState<ProductInput>(() => initialInput(product));
  const [errors, setErrors] = useState<ReturnType<typeof validateProduct>>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  function update(field: keyof ProductInput, value: string) {
    setInput((current) => ({ ...current, [field]: value }));
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (saving) {
      return;
    }
    const nextErrors = validateProduct(input);
    setErrors(nextErrors);
    setFormError(null);
    if (Object.keys(nextErrors).length > 0) {
      return;
    }
    setSaving(true);
    try {
      const saved = product ? await updateProduct(product.id, input) : await createProduct(input);
      onSaved(saved);
    } catch (error) {
      setFormError(toErrorMessage(error));
      setSaving(false);
    }
  }

  return (
    <form onSubmit={submit} className="panel max-w-3xl" noValidate>
      <div className="panel-header">
        <h2 className="panel-title">{product ? "Modifier le produit" : "Nouveau produit"}</h2>
        <RequiredLegend />
      </div>
      <div className="grid gap-x-4 gap-y-4 p-4 sm:grid-cols-2">
        <Field id="product-name" label="Nom du produit" required error={errors.name} className="sm:col-span-2">
          <input
            id="product-name"
            value={input.name}
            aria-invalid={Boolean(errors.name)}
            aria-describedby={errors.name ? "product-name-error" : undefined}
            onChange={(event) => update("name", event.target.value)}
            className={controlClass}
            autoFocus
          />
        </Field>
        <Field id="product-reference" label="Référence" error={errors.reference} hint="Code article interne ou fournisseur.">
          <input
            id="product-reference"
            value={input.reference}
            aria-invalid={Boolean(errors.reference)}
            aria-describedby={errors.reference ? "product-reference-error" : undefined}
            onChange={(event) => update("reference", event.target.value)}
            className={controlClass}
          />
        </Field>
        <Field id="product-type" label="Type de produit" required error={errors.typeId}>
          <select
            id="product-type"
            value={input.typeId}
            aria-invalid={Boolean(errors.typeId)}
            aria-describedby={errors.typeId ? "product-type-error" : undefined}
            onChange={(event) => update("typeId", event.target.value)}
            className={controlClass}
          >
            <option value="">Choisir un type</option>
            {options(types, input.typeId).map((type) => (
              <option key={type.id} value={type.id}>
                {type.name}
                {type.isActive ? "" : " (désactivé)"}
              </option>
            ))}
          </select>
        </Field>
        <Field id="product-supplier" label="Fournisseur principal" className="sm:col-span-2">
          <select
            id="product-supplier"
            value={input.supplierId}
            onChange={(event) => update("supplierId", event.target.value)}
            className={controlClass}
          >
            <option value="">Aucun fournisseur</option>
            {options(suppliers, input.supplierId).map((supplier) => (
              <option key={supplier.id} value={supplier.id}>
                {supplier.name}
                {supplier.isActive ? "" : " (désactivé)"}
              </option>
            ))}
          </select>
        </Field>
        <Field id="product-description" label="Description" error={errors.description} className="sm:col-span-2">
          <textarea
            id="product-description"
            value={input.description}
            rows={4}
            aria-invalid={Boolean(errors.description)}
            aria-describedby={errors.description ? "product-description-error" : undefined}
            onChange={(event) => update("description", event.target.value)}
            className={controlClass}
          />
        </Field>
        {formError ? (
          <Callout tone="danger" role="alert" className="sm:col-span-2">
            {formError}
          </Callout>
        ) : null}
      </div>
      <div className="flex justify-end gap-2 rounded-b-panel border-t border-line bg-surface-muted px-4 py-3">
        <button type="button" onClick={onCancel} disabled={saving} className={quietButtonClass}>
          Annuler
        </button>
        <button type="submit" disabled={saving} className={buttonClass}>
          {saving ? <Spinner /> : null}
          {saving ? "Enregistrement…" : "Enregistrer"}
        </button>
      </div>
    </form>
  );
}
