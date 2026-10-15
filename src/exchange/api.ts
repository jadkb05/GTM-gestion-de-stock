import { invoke } from "@tauri-apps/api/core";
import type { ProductQuery } from "../catalogue/types";

export type ExportFormat = "csv" | "xlsx" | "pdf";

export type ExportOutcome = {
  rows: number;
  scope: string;
};

export type ImportReady = {
  line: number;
  name: string;
  reference: string | null;
  typeName: string;
  supplierName: string | null;
  description: string | null;
};

export type ImportIssue = {
  line: number;
  message: string;
};

export type ImportPreview = {
  ready: ImportReady[];
  issues: ImportIssue[];
  ignoredColumns: string[];
};

function optionalId(value: string): number | null {
  if (value.trim().length === 0) {
    return null;
  }
  return Number(value);
}

export function exportCatalogue(
  path: string,
  format: ExportFormat,
  query: ProductQuery,
): Promise<ExportOutcome> {
  return invoke<ExportOutcome>("export_catalogue", {
    path,
    formatName: format,
    query: {
      search: query.search.trim(),
      typeId: optionalId(query.typeId),
      supplierId: optionalId(query.supplierId),
      status: query.status,
    },
  });
}

export function exportHistory(
  path: string,
  format: ExportFormat,
  query: { search: string; productId: string; direction: string; fromDate: string; toDate: string },
): Promise<ExportOutcome> {
  return invoke<ExportOutcome>("export_history", {
    path,
    formatName: format,
    query: {
      search: query.search.trim(),
      productId: optionalId(query.productId),
      direction: query.direction,
      fromDate: query.fromDate.trim() || null,
      toDate: query.toDate.trim() || null,
    },
  });
}

export function exportInventory(path: string, format: ExportFormat, inventoryId: number): Promise<ExportOutcome> {
  return invoke<ExportOutcome>("export_inventory", { path, formatName: format, inventoryId });
}

export function exportReorderAlerts(
  path: string,
  format: ExportFormat,
  query: { search: string; typeId: string; supplierId: string; sort: string },
): Promise<ExportOutcome> {
  return invoke<ExportOutcome>("export_reorder_alerts", {
    path,
    formatName: format,
    query: {
      search: query.search.trim(),
      typeId: optionalId(query.typeId),
      supplierId: optionalId(query.supplierId),
      sort: query.sort,
    },
  });
}

export function previewArticleImport(path: string): Promise<ImportPreview> {
  return invoke<ImportPreview>("preview_article_import", { path });
}

export function commitArticleImport(path: string): Promise<{ created: number }> {
  return invoke<{ created: number }>("commit_article_import", { path });
}
