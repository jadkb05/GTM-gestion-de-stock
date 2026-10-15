import { useEffect, useState } from "react";
import { toErrorMessage } from "../app/api";
import { createInventory, listInventories } from "../inventory/api";
import type { InventoryListItem } from "../inventory/api";
import { formatDateTime } from "../app/format";
import { buttonClass, controlClass, FilterField, quietButtonClass } from "./Field";
import { StatusBadge } from "./StatusBadge";
import { Dialog } from "./ui/Dialog";
import { Callout } from "./ui/Feedback";
import { Icon } from "./ui/Icon";
import { EmptyState, Loading, PageHeader } from "./ui/Layout";
import { InventoryDetail } from "./InventoryDetail";

type InventoriesProps = {
  inventoryId: number | null;
  onOpen: (id: number) => void;
  onBack: () => void;
  onOpenSheet: (productId: number) => void;
  onStockChanged: () => void;
};

function statusLabel(status: string) {
  return status === "validated" ? "Validé" : "Brouillon";
}

function formatWhen(value: string | null) {
  return formatDateTime(value);
}

export function Inventories({ inventoryId, onOpen, onBack, onOpenSheet, onStockChanged }: InventoriesProps) {
  const [items, setItems] = useState<InventoryListItem[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [comment, setComment] = useState("");
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    if (inventoryId !== null) {
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    listInventories()
      .then((next) => {
        if (!cancelled) {
          setItems(next);
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
  }, [inventoryId, revision]);

  async function create() {
    setError(null);
    try {
      const created = await createInventory({
        name: name.trim().length === 0 ? null : name.trim(),
        comment: comment.trim().length === 0 ? null : comment.trim(),
      });
      setName("");
      setComment("");
      setCreating(false);
      setRevision((current) => current + 1);
      onOpen(created.id);
    } catch (reason) {
      setError(toErrorMessage(reason));
    }
  }

  if (inventoryId !== null) {
    return (
      <InventoryDetail
        inventoryId={inventoryId}
        onBack={onBack}
        onOpenSheet={onOpenSheet}
        onStockChanged={onStockChanged}
      />
    );
  }

  return (
    <section>
      <PageHeader
        title="Inventaires"
        description="Comptages physiques. Le stock de référence est figé à l'ajout de chaque produit ; la validation crée les corrections d'écart."
        actions={
          <button type="button" className={buttonClass} onClick={() => setCreating((current) => !current)}>
            <Icon name="plus" size={16} />
            Nouvel inventaire
          </button>
        }
      />
      {creating ? (
        <Dialog
          title="Nouvel inventaire"
          onClose={() => setCreating(false)}
          footer={
            <>
              <button type="button" className={quietButtonClass} onClick={() => setCreating(false)}>
                Annuler
              </button>
              <button type="submit" form="inventory-create" className={buttonClass}>
                Créer l'inventaire
              </button>
            </>
          }
        >
          <form
            id="inventory-create"
            className="grid gap-4"
            onSubmit={(event) => {
              event.preventDefault();
              void create();
            }}
          >
            <FilterField label="Nom ou référence">
              <input
                value={name}
                onChange={(event) => setName(event.target.value)}
                className={controlClass}
                placeholder="Exemple : Dépôt — zone A"
              />
            </FilterField>
            <FilterField label="Commentaire">
              <textarea value={comment} onChange={(event) => setComment(event.target.value)} className={controlClass} rows={3} />
            </FilterField>
            {error ? (
              <Callout tone="danger" role="alert">
                {error}
              </Callout>
            ) : null}
          </form>
        </Dialog>
      ) : null}
      {error && !creating ? (
        <Callout tone="danger" role="alert" className="mb-4">
          {error}
        </Callout>
      ) : null}
      {loading ? <Loading>Chargement des inventaires…</Loading> : null}
      {!loading && items.length === 0 ? (
        <EmptyState icon="inventory" title="Aucun inventaire enregistré.">
          Créez un inventaire, ajoutez les produits à compter puis saisissez les quantités relevées.
        </EmptyState>
      ) : null}
      {items.length > 0 ? (
        <div className="table-frame table-frame-tall">
          <table className="data-table min-w-[48rem]">
            <thead>
              <tr>
                <th>Inventaire</th>
                <th>Statut</th>
                <th>Créé le</th>
                <th>Validé le</th>
                <th className="num">Produits</th>
                <th className="num">Écarts</th>
                <th className="actions">Actions</th>
              </tr>
            </thead>
            <tbody>
              {items.map((item) => (
                <tr key={item.id}>
                  <td className="font-medium">{item.name ?? `Inventaire ${item.id}`}</td>
                  <td>
                    <StatusBadge tone={item.status === "validated" ? "ok" : "info"}>{statusLabel(item.status)}</StatusBadge>
                  </td>
                  <td className="nowrap tabular-nums text-mist">{formatWhen(item.createdAt)}</td>
                  <td className="nowrap tabular-nums text-mist">{formatWhen(item.validatedAt)}</td>
                  <td className="num">{item.productCount}</td>
                  <td className={`num ${item.varianceCount > 0 ? "font-semibold text-warn" : ""}`}>{item.varianceCount}</td>
                  <td className="actions">
                    <button type="button" className="btn btn-secondary btn-sm" onClick={() => onOpen(item.id)}>
                      Détail
                      <Icon name="chevronRight" size={14} />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </section>
  );
}
