import type { SectionId } from "../app/navigation";
import type { LoadState } from "../app/types";
import type { BackupStatus } from "../backup/api";
import type { ReorderAlert } from "../inventory/api";
import type { StockOverview } from "../stock/api";
import { AlertsPage } from "./AlertsPage";
import { BackupPage } from "./BackupPage";
import { Dashboard } from "./Dashboard";
import { ErrorState } from "./ErrorState";
import { Inventories } from "./Inventories";
import { MovementForm } from "./MovementForm";
import { MovementHistory } from "./MovementHistory";
import { ProductCatalogue } from "./ProductCatalogue";
import { StockSheet } from "./StockSheet";
import { ProductTypesPage } from "./ProductTypesPage";
import { Sidebar } from "./Sidebar";
import { SuppliersPage } from "./SuppliersPage";

type AppShellProps = {
  section: SectionId;
  onSectionChange: (section: SectionId) => void;
  movementProductId: number | null;
  onOpenMovement: (section: "entries" | "exits", productId: number) => void;
  sheetProductId: number | null;
  sheetOrigin: "catalogue" | "history" | "alerts" | "inventory" | "dashboard";
  onOpenSheet: (productId: number, origin: "catalogue" | "history" | "alerts" | "inventory" | "dashboard") => void;
  inventoryId: number | null;
  onOpenInventory: (id: number) => void;
  onBackToInventories: () => void;
  onReturnToInventory: () => void;
  alertCount: number | null;
  overview: StockOverview | null;
  overviewError: string | null;
  attention?: ReorderAlert[] | null;
  backupStatus?: BackupStatus | null;
  onStockChanged: () => void;
  onRestored: () => void;
  state: LoadState;
  onRetry: () => void;
};

export function AppShell({
  section,
  onSectionChange,
  movementProductId,
  onOpenMovement,
  sheetProductId,
  sheetOrigin,
  onOpenSheet,
  inventoryId,
  onOpenInventory,
  onBackToInventories,
  onReturnToInventory,
  alertCount,
  overview,
  overviewError,
  attention = null,
  backupStatus = null,
  onStockChanged,
  onRestored,
  state,
  onRetry,
}: AppShellProps) {
  let page;
  if (section === "dashboard") {
    page =
      state.status === "error" ? (
        <ErrorState message={state.message} onRetry={onRetry} />
      ) : (
        <Dashboard
          state={state.status === "ready" ? "ready" : "loading"}
          data={state.status === "ready" ? state.data : undefined}
          overview={overview}
          overviewError={overviewError}
          attention={attention}
          backupStatus={backupStatus}
          onNavigate={onSectionChange}
          onOpenMovement={(productId) => onOpenSheet(productId, "dashboard")}
        />
      );
  } else if (section === "catalogue") {
    page = (
      <ProductCatalogue
        onEntry={(productId) => onOpenMovement("entries", productId)}
        onExit={(productId) => onOpenMovement("exits", productId)}
        onOpenSheet={(productId) => onOpenSheet(productId, "catalogue")}
      />
    );
  } else if (section === "entries") {
    page = (
      <MovementForm direction="in" initialProductId={movementProductId} onRecorded={onStockChanged} />
    );
  } else if (section === "exits") {
    page = (
      <MovementForm direction="out" initialProductId={movementProductId} onRecorded={onStockChanged} />
    );
  } else if (section === "history") {
    page = <MovementHistory onOpenSheet={(productId) => onOpenSheet(productId, "history")} />;
  } else if (section === "sheet" && sheetProductId !== null) {
    page = (
      <StockSheet
        productId={sheetProductId}
        backLabel={
          sheetOrigin === "history"
            ? "Retour à l'historique"
            : sheetOrigin === "alerts"
              ? "Retour au réapprovisionnement"
              : sheetOrigin === "inventory"
                ? "Retour à l'inventaire"
                : sheetOrigin === "dashboard"
                  ? "Retour au tableau de bord"
                  : "Retour au catalogue"
        }
        onBack={() => {
          if (sheetOrigin === "inventory") {
            onReturnToInventory();
            return;
          }
          onSectionChange(sheetOrigin);
        }}
        onStockChanged={onStockChanged}
      />
    );
  } else if (section === "inventory") {
    page = (
      <Inventories
        inventoryId={inventoryId}
        onOpen={onOpenInventory}
        onBack={onBackToInventories}
        onOpenSheet={(productId) => onOpenSheet(productId, "inventory")}
        onStockChanged={onStockChanged}
      />
    );
  } else if (section === "alerts") {
    page = (
      <AlertsPage
        onOpenSheet={(productId) => onOpenSheet(productId, "alerts")}
        onOpenEntry={(productId) => onOpenMovement("entries", productId)}
        onStockChanged={onStockChanged}
      />
    );
  } else if (section === "backup") {
    page = <BackupPage onRestored={onRestored} onBackupCreated={onStockChanged} />;
  } else if (section === "product-types") {
    page = <ProductTypesPage />;
  } else if (section === "suppliers") {
    page = <SuppliersPage />;
  } else {
    page = (
      <p className="text-table text-mist">Ouvrez une fiche depuis le catalogue, l'historique ou le réapprovisionnement.</p>
    );
  }

  return (
    <div className="flex h-full overflow-hidden">
      <Sidebar
        current={section}
        context={section === "sheet" ? sheetOrigin : undefined}
        onSelect={onSectionChange}
        alertCount={alertCount}
      />
      <main className="min-w-0 flex-1 overflow-auto">
        <div className="mx-auto w-full max-w-[90rem] px-7 pb-10 pt-6">{page}</div>
      </main>
    </div>
  );
}
