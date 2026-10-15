import { invoke } from "@tauri-apps/api/core";

export type BackupReport = {
  schemaVersion: number;
  appName: string;
  appVersion: string | null;
  createdAt: string | null;
  formatVersion: string | null;
  productCount: number;
  supplierCount: number;
  typeCount: number;
  movementCount: number;
  inventoryCount: number;
};

export type BackupStatus = {
  createdAt: string | null;
  path: string | null;
};

export function getBackupStatus(): Promise<BackupStatus> {
  return invoke<BackupStatus>("get_backup_status");
}

export function createBackup(path: string): Promise<BackupReport> {
  return invoke<BackupReport>("create_backup", { path });
}

export function inspectBackup(path: string): Promise<BackupReport> {
  return invoke<BackupReport>("inspect_backup", { path });
}

export function restoreBackup(path: string): Promise<BackupReport> {
  return invoke<BackupReport>("restore_backup", { path });
}
