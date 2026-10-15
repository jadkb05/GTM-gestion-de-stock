import { useEffect, useState } from "react";
import type { SectionId } from "./app/navigation";
import { useAppStatus } from "./app/useAppStatus";
import { AppShell } from "./components/AppShell";
import { getBackupStatus, type BackupStatus } from "./backup/api";
import { listReorderAlerts, type ReorderAlert } from "./inventory/api";
import { getStockOverview, type StockOverview } from "./stock/api";

function App() {
  const [section, setSection] = useState<SectionId>("dashboard");
  const [movementProductId, setMovementProductId] = useState<number | null>(null);
  const [sheetProductId, setSheetProductId] = useState<number | null>(null);
  const [sheetOrigin, setSheetOrigin] = useState<
    "catalogue" | "history" | "alerts" | "inventory" | "dashboard"
  >("catalogue");
  const [inventoryId, setInventoryId] = useState<number | null>(null);
  const [alertCount, setAlertCount] = useState<number | null>(null);
  const [overview, setOverview] = useState<StockOverview | null>(null);
  const [overviewError, setOverviewError] = useState<string | null>(null);
  const [attention, setAttention] = useState<ReorderAlert[] | null>(null);
  const [backupStatus, setBackupStatus] = useState<BackupStatus | null>(null);
  const [stockRevision, setStockRevision] = useState(0);
  const { state, reload } = useAppStatus();
  const ready = state.status === "ready";

  useEffect(() => {
    if (!ready) {
      setAlertCount(null);
      setOverview(null);
      setOverviewError(null);
      setAttention(null);
      setBackupStatus(null);
      return;
    }
    setOverviewError(null);
    let cancelled = false;
    getStockOverview()
      .then((value) => {
        if (cancelled || !isOverview(value)) {
          return;
        }
        setOverview(value);
        setAlertCount(value.reorderAlerts);
      })
      .catch(() => {
        if (!cancelled) {
          setOverview(null);
          setAlertCount(null);
          setOverviewError("Les indicateurs n'ont pas pu être chargés.");
        }
      });
    // Articles à surveiller sur le tableau de bord : mêmes règles que la page de réapprovisionnement.
    listReorderAlerts({ search: "", typeId: "", supplierId: "", sort: "asc" })
      .then((rows) => {
        if (!cancelled) {
          setAttention(Array.isArray(rows) ? rows : null);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setAttention(null);
        }
      });
    // Rappel de sauvegarde : la base n'existe que sur ce poste.
    getBackupStatus()
      .then((value) => {
        if (!cancelled) {
          setBackupStatus(isBackupStatus(value) ? value : null);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setBackupStatus(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [ready, stockRevision]);

  function changeSection(next: SectionId) {
    setMovementProductId(null);
    setInventoryId(null);
    setSection(next);
  }

  function openMovement(next: "entries" | "exits", productId: number) {
    setMovementProductId(productId);
    setSection(next);
  }

  function openSheet(
    productId: number,
    origin: "catalogue" | "history" | "alerts" | "inventory" | "dashboard",
  ) {
    setSheetProductId(productId);
    setSheetOrigin(origin);
    setSection("sheet");
  }

  function notifyStockChanged() {
    setStockRevision((current) => current + 1);
  }

  return (
    <AppShell
      section={section}
      onSectionChange={changeSection}
      movementProductId={movementProductId}
      onOpenMovement={openMovement}
      sheetProductId={sheetProductId}
      sheetOrigin={sheetOrigin}
      onOpenSheet={openSheet}
      inventoryId={inventoryId}
      onOpenInventory={(id) => {
        setInventoryId(id);
        setSection("inventory");
      }}
      onBackToInventories={() => {
        setInventoryId(null);
        setSection("inventory");
      }}
      onReturnToInventory={() => setSection("inventory")}
      alertCount={alertCount}
      overview={overview}
      overviewError={overviewError}
      attention={attention}
      backupStatus={backupStatus}
      onStockChanged={notifyStockChanged}
      onRestored={() => {
        notifyStockChanged();
        reload();
      }}
      state={state}
      onRetry={reload}
    />
  );
}

function isBackupStatus(value: unknown): value is BackupStatus {
  return Boolean(value && typeof value === "object" && "createdAt" in value && "path" in value);
}

function isOverview(value: unknown): value is StockOverview {
  if (!value || typeof value !== "object") {
    return false;
  }
  const row = value as StockOverview;
  return typeof row.activeProducts === "number" && Array.isArray(row.recentMovements);
}

export default App;
