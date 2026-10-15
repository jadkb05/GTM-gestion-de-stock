import { invoke } from "@tauri-apps/api/core";

export type StockLevel = {
  productId: number;
  quantity: string;
};

export type ProductStock = {
  productId: number;
  productName: string;
  isActive: boolean;
  quantity: string;
};

export type StockMovement = {
  id: number;
  productId: number;
  productName: string;
  productReference: string | null;
  direction: "in" | "out";
  quantity: string;
  reason: string | null;
  documentReference: string | null;
  comment: string | null;
  createdAt: string;
  source?: string;
  inventoryId?: number | null;
  balanceAfter?: string;
};

export type StockSheet = {
  productId: number;
  name: string;
  reference: string | null;
  typeName: string;
  supplierName: string | null;
  isActive: boolean;
  quantity: string;
  movementCount: number;
  entriesTotal: string;
  exitsTotal: string;
  reorderThreshold?: string | null;
};

export type SheetLine = {
  id: number;
  createdAt: string;
  direction: "in" | "out";
  quantity: string;
  balanceAfter: string;
  reason: string | null;
  documentReference: string | null;
  comment: string | null;
  source?: string;
};

export type SheetPage = {
  lines: SheetLine[];
  page: number;
  pageSize: number;
  total: number;
};

export type MovementInput = {
  productId: string;
  quantity: string;
  reason: string;
  documentReference: string;
  comment: string;
};

export type MovementQuery = {
  search: string;
  productId: string;
  direction: "all" | "in" | "out";
};

function payload(input: MovementInput) {
  const text = (value: string) => {
    const trimmed = value.trim();
    return trimmed.length === 0 ? null : trimmed;
  };
  return {
    productId: Number(input.productId),
    quantity: input.quantity.trim().replace(",", "."),
    reason: text(input.reason),
    documentReference: text(input.documentReference),
    comment: text(input.comment),
  };
}

export type StockOverview = {
  activeProducts: number;
  outOfStock: number;
  reorderAlerts: number;
  movementsLast7Days: number;
  recentMovements: StockMovement[];
};

export function getStockOverview(): Promise<StockOverview> {
  return invoke<StockOverview>("get_stock_overview");
}

export function listStockLevels(): Promise<StockLevel[]> {
  return invoke<StockLevel[]>("list_stock_levels");
}

export function getProductStock(productId: number): Promise<ProductStock> {
  return invoke<ProductStock>("get_product_stock", { productId });
}

export function recordStockEntry(input: MovementInput): Promise<StockMovement> {
  return invoke<StockMovement>("record_stock_entry", { movement: payload(input) });
}

export function recordStockExit(input: MovementInput): Promise<StockMovement> {
  return invoke<StockMovement>("record_stock_exit", { movement: payload(input) });
}

export function listStockMovements(query: MovementQuery): Promise<StockMovement[]> {
  return invoke<StockMovement[]>("list_stock_movements", {
    query: {
      search: query.search.trim(),
      productId: query.productId.trim().length === 0 ? null : Number(query.productId),
      direction: query.direction,
    },
  });
}

export function getStockMovement(id: number): Promise<StockMovement> {
  return invoke<StockMovement>("get_stock_movement", { id });
}

export function getStockSheet(productId: number): Promise<StockSheet> {
  return invoke<StockSheet>("get_stock_sheet", { productId });
}

export function listStockSheetMovements(query: {
  productId: number;
  direction: "all" | "in" | "out";
  search: string;
  fromDate: string;
  toDate: string;
  page: number;
}): Promise<SheetPage> {
  return invoke<SheetPage>("list_stock_sheet_movements", {
    query: {
      productId: query.productId,
      direction: query.direction,
      search: query.search.trim(),
      fromDate: query.fromDate.trim() || null,
      toDate: query.toDate.trim() || null,
      page: query.page,
    },
  });
}
