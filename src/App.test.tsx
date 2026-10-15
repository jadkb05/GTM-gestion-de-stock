import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const readyStatus = {
  databasePath: "/home/exemple/.local/share/com.gtm.stock-manager/gtm-stock.db",
  schemaVersion: 1,
  appName: "GTM Stock Manager",
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("shell de l'application", () => {
  it("affiche le titre et l'état réel de la base", async () => {
    vi.mocked(invoke).mockResolvedValue(readyStatus);

    render(<App />);

    expect(await screen.findByRole("heading", { name: "Tableau de bord" })).toBeTruthy();
    expect(screen.getByText("Initialisée")).toBeTruthy();
    expect(screen.getByText("1")).toBeTruthy();
    expect(screen.getByText(readyStatus.databasePath)).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("get_app_status");
  });

  it("affiche l'erreur puis permet de réessayer", async () => {
    vi.mocked(invoke)
      .mockRejectedValueOnce("Impossible d'ouvrir la base locale : disque plein")
      .mockResolvedValueOnce(readyStatus);

    render(<App />);

    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByText("Impossible d'ouvrir la base locale : disque plein")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Réessayer" }));

    expect(await screen.findByText("Initialisée")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("get_app_status");
  });

  it("ouvre les inventaires sans les présenter comme indisponibles", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "get_app_status") {
        return readyStatus;
      }
      if (command === "get_stock_overview") {
        return {
          activeProducts: 0,
          outOfStock: 0,
          reorderAlerts: 0,
          movementsLast7Days: 0,
          recentMovements: [],
        };
      }
      if (command === "list_inventories") {
        return [];
      }
      return [];
    });

    render(<App />);
    await screen.findByRole("heading", { name: "Tableau de bord" });

    fireEvent.click(within(screen.getByRole("navigation", { name: "Modules" })).getByRole("button", { name: "Inventaires" }));

    expect(await screen.findByRole("heading", { name: "Inventaires" })).toBeTruthy();
    expect(screen.getByText("Aucun inventaire enregistré.")).toBeTruthy();
    expect(screen.queryByText(/pas encore disponible/i)).toBeNull();
    expect(screen.queryByText("Indisponible")).toBeNull();
  });
});
