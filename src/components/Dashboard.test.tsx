import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppStatus } from "../app/types";
import type { StockOverview } from "../stock/api";
import { Dashboard } from "./Dashboard";

const status: AppStatus = {
  databasePath: "/tmp/gtm-stock.db",
  schemaVersion: 4,
  appName: "GTM Stock Manager",
};

const overview: StockOverview = {
  activeProducts: 4,
  outOfStock: 2,
  reorderAlerts: 1,
  movementsLast7Days: 3,
  recentMovements: [
    {
      id: 9,
      productId: 12,
      productName: "Colle",
      productReference: "COL-1",
      direction: "in",
      quantity: "3",
      reason: null,
      documentReference: null,
      comment: null,
      createdAt: "2026-10-13T08:00:00Z",
      source: "manual",
    },
  ],
};

afterEach(cleanup);

function figure(label: string) {
  return screen.getByText(label).parentElement?.querySelector("dd")?.textContent;
}

describe("tableau de bord", () => {
  it("sépare les ruptures des alertes et ouvre les opérations", () => {
    const onNavigate = vi.fn();
    const onOpenMovement = vi.fn();
    render(
      <Dashboard
        state="ready"
        data={status}
        overview={overview}
        overviewError={null}
        onNavigate={onNavigate}
        onOpenMovement={onOpenMovement}
      />,
    );

    expect(figure("Produits actifs")).toBe("4");
    expect(figure("Ruptures de stock")).toBe("2");
    expect(figure("À réapprovisionner")).toBe("1");
    expect(screen.getByText("Stock disponible égal à zéro.")).toBeTruthy();
    expect(screen.getByText(/même s'il reste du stock/)).toBeTruthy();
    expect(screen.getByText("Colle")).toBeTruthy();
    expect(screen.getByText("Entrée")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Nouvelle entrée" }));
    fireEvent.click(screen.getByRole("button", { name: "Colle" }));
    expect(onNavigate).toHaveBeenCalledWith("entries");
    expect(onOpenMovement).toHaveBeenCalledWith(12);
  });

  it("indique l'absence de mouvement", () => {
    render(
      <Dashboard
        state="ready"
        data={status}
        overview={{ ...overview, recentMovements: [], outOfStock: 0, reorderAlerts: 0 }}
        overviewError={null}
        onNavigate={() => undefined}
        onOpenMovement={() => undefined}
      />,
    );
    expect(screen.getByText("Aucun mouvement enregistré.")).toBeTruthy();
  });

  it("affiche l'erreur de chargement des indicateurs", () => {
    render(
      <Dashboard
        state="ready"
        data={status}
        overview={null}
        overviewError="Les indicateurs n'ont pas pu être chargés."
        onNavigate={() => undefined}
        onOpenMovement={() => undefined}
      />,
    );
    expect(screen.getByRole("alert").textContent).toContain("Les indicateurs n'ont pas pu être chargés.");
    expect(screen.queryByText("Ruptures de stock")).toBeNull();
  });
});

describe("rappel de sauvegarde", () => {
  it("invite à sauvegarder lorsqu'aucune sauvegarde n'existe", () => {
    const onNavigate = vi.fn();
    render(
      <Dashboard
        state="ready"
        data={status}
        overview={overview}
        overviewError={null}
        backupStatus={{ createdAt: null, path: null }}
        onNavigate={onNavigate}
        onOpenMovement={() => undefined}
      />,
    );
    expect(screen.getByText(/Aucune sauvegarde n'a encore été réalisée/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Créer une sauvegarde" }));
    expect(onNavigate).toHaveBeenCalledWith("backup");
  });

  it("reste discret après une sauvegarde récente", () => {
    render(
      <Dashboard
        state="ready"
        data={status}
        overview={overview}
        overviewError={null}
        backupStatus={{ createdAt: new Date().toISOString(), path: "/media/cle/gtm.gtm-backup" }}
        onNavigate={() => undefined}
        onOpenMovement={() => undefined}
      />,
    );
    expect(screen.queryByText("Sauvegarde recommandée")).toBeNull();
  });
});
