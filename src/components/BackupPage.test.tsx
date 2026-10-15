import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BackupPage } from "./BackupPage";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(),
  open: vi.fn(),
}));

const report = {
  schemaVersion: 4,
  appName: "GTM Stock Manager",
  appVersion: "0.1.0",
  createdAt: "2026-10-14T13:00:00.000Z",
  formatVersion: "1",
  productCount: 2,
  supplierCount: 3,
  typeCount: 3,
  movementCount: 4,
  inventoryCount: 1,
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function mockInvoke() {
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "get_backup_status") {
      return { createdAt: null, path: null };
    }
    if (command === "create_backup" || command === "inspect_backup" || command === "restore_backup") {
      return report;
    }
    return null;
  });
}

describe("sauvegarde", () => {
  it("crée une sauvegarde au chemin choisi", async () => {
    mockInvoke();
    vi.mocked(save).mockResolvedValue("/tmp/sauvegarde-gtm.gtm-backup");
    render(<BackupPage onRestored={() => undefined} />);
    expect(await screen.findByText(/aucune pour le moment/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Créer une sauvegarde" }));
    expect(await screen.findByRole("status")).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("/tmp/sauvegarde-gtm.gtm-backup");
    expect(invoke).toHaveBeenCalledWith("create_backup", { path: "/tmp/sauvegarde-gtm.gtm-backup" });
  });

  it("n'appelle pas la restauration si l'utilisateur annule", async () => {
    mockInvoke();
    vi.mocked(open).mockResolvedValue("/tmp/ancienne.gtm-backup");
    render(<BackupPage onRestored={() => undefined} />);
    fireEvent.click(screen.getByRole("button", { name: "Restaurer une sauvegarde" }));
    expect(await screen.findByText(/remplace toutes les données/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(screen.queryByText(/remplace toutes les données/)).toBeNull();
    expect(invoke).not.toHaveBeenCalledWith("restore_backup", expect.anything());
  });

  it("restaure seulement après confirmation", async () => {
    mockInvoke();
    vi.mocked(open).mockResolvedValue("/tmp/ancienne.gtm-backup");
    const onRestored = vi.fn();
    render(<BackupPage onRestored={onRestored} />);
    fireEvent.click(screen.getByRole("button", { name: "Restaurer une sauvegarde" }));
    expect(await screen.findByText(/remplace toutes les données/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Confirmer le remplacement" }));
    expect(await screen.findByText(/Restauration terminée/)).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("restore_backup", { path: "/tmp/ancienne.gtm-backup" });
    expect(onRestored).toHaveBeenCalled();
  });
});
