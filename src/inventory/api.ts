import { invoke } from "@tauri-apps/api/core";

export type InventoryListItem = {
  id: number;
  name: string | null;
  status: "draft" | "validated";
  createdAt: string;
  validatedAt: string | null;
  productCount: number;
  varianceCount: number;
};

export type InventoryCounts = {
  counted: number;
  unchanged: number;
  surplus: number;
  shortage: number;
  uncounted: number;
  stale: number;
};

export type InventoryLine = {
  id: number;
  productId: number;
  productName: string;
  productReference: string | null;
  typeName: string;
  theoretical: string;
  counted: string | null;
  current: string;
  variance: string | null;
  drifted: boolean;
};

export type InventoryDetail = {
  id: number;
  name: string | null;
  comment: string | null;
  status: "draft" | "validated";
  createdAt: string;
  validatedAt: string | null;
  lines: InventoryLine[];
  counts: InventoryCounts;
};

export type ReorderAlert = {
  productId: number;
  name: string;
  reference: string | null;
  typeId: number;
  typeName: string;
  supplierId: number | null;
  supplierName: string | null;
  quantity: string;
  threshold: string;
  gap: string;
};

export function listInventories(): Promise<InventoryListItem[]> {
  return invoke<InventoryListItem[]>("list_inventories");
}

export function createInventory(input: {
  name: string | null;
  comment: string | null;
}): Promise<InventoryDetail> {
  return invoke<InventoryDetail>("create_inventory", { inventory: input });
}

export function getInventory(id: number): Promise<InventoryDetail> {
  return invoke<InventoryDetail>("get_inventory", { id });
}

export function addInventoryProducts(inventoryId: number, productIds: number[]): Promise<InventoryDetail> {
  return invoke<InventoryDetail>("add_inventory_products", { inventoryId, productIds });
}

export function saveInventoryCounts(
  inventoryId: number,
  counts: { lineId: number; quantity: string | null }[],
): Promise<InventoryDetail> {
  return invoke<InventoryDetail>("save_inventory_counts", { inventoryId, counts });
}

export function refreshInventoryLine(lineId: number, resetCount: boolean): Promise<InventoryDetail> {
  return invoke<InventoryDetail>("refresh_inventory_line", { lineId, resetCount });
}

export function removeInventoryLine(lineId: number): Promise<InventoryDetail> {
  return invoke<InventoryDetail>("remove_inventory_line", { lineId });
}

export function validateInventory(id: number): Promise<InventoryDetail> {
  return invoke<InventoryDetail>("validate_inventory", { id });
}

export function listReorderAlerts(query: {
  search: string;
  typeId: string;
  supplierId: string;
  sort: "asc" | "desc";
}): Promise<ReorderAlert[]> {
  const typeId = query.typeId.trim();
  const supplierId = query.supplierId.trim();
  return invoke<ReorderAlert[]>("list_reorder_alerts", {
    query: {
      search: query.search.trim(),
      typeId: typeId.length === 0 ? null : Number(typeId),
      supplierId: supplierId.length === 0 ? null : Number(supplierId),
      sort: query.sort,
    },
  });
}

export function setReorderThreshold(productId: number, threshold: string | null): Promise<string | null> {
  return invoke<string | null>("set_reorder_threshold", {
    productId,
    threshold,
  });
}

export function countReorderAlerts(): Promise<number> {
  return invoke<number>("count_reorder_alerts");
}
