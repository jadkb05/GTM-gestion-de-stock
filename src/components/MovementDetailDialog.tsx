import { formatDateTime } from "../app/format";
import type { StockMovement } from "../stock/api";
import { displayQuantity, movementLabel } from "../stock/quantity";
import { Dialog } from "./ui/Dialog";
import { MovementTag } from "./ui/Feedback";
import { Detail } from "./ui/Layout";

type MovementDetailDialogProps = {
  movement: StockMovement;
  onClose: () => void;
};

/* Détail complet d'un mouvement, consulté depuis l'historique ou une fiche de stock. */
export function MovementDetailDialog({ movement, onClose }: MovementDetailDialogProps) {
  return (
    <Dialog
      title="Détail du mouvement"
      onClose={onClose}
      footer={
        <button type="button" className="btn btn-secondary" onClick={onClose}>
          Fermer le détail
        </button>
      }
    >
      <dl className="grid grid-cols-2 gap-x-6 gap-y-4 text-table">
        <Detail label="Produit" className="col-span-2">
          {movement.productName}
          {movement.productReference ? (
            <span className="ml-2 font-normal text-mist">{movement.productReference}</span>
          ) : null}
        </Detail>
        <Detail label="Mouvement">
          <MovementTag direction={movement.direction} label={movementLabel(movement.direction, movement.source)} />
        </Detail>
        <Detail label="Identifiant">
          <span className="tabular-nums">{movement.id}</span>
        </Detail>
        <Detail label="Quantité">
          <span className="text-section tabular-nums">{displayQuantity(movement.quantity)}</span>
        </Detail>
        <Detail label="Solde après mouvement">
          <span className="text-section tabular-nums">
            {movement.balanceAfter ? displayQuantity(movement.balanceAfter) : "—"}
          </span>
        </Detail>
        <Detail label="Date">
          <span className="tabular-nums">{formatDateTime(movement.createdAt, true)}</span>
        </Detail>
        <Detail label="Référence documentaire">{movement.documentReference ?? "—"}</Detail>
        <Detail label="Motif" className="col-span-2">
          {movement.reason ?? "—"}
        </Detail>
        <Detail label="Commentaire" className="col-span-2">
          <span className="whitespace-pre-wrap font-normal">{movement.comment ?? "—"}</span>
        </Detail>
      </dl>
    </Dialog>
  );
}
