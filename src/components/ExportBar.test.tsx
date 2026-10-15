import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { act } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ExportBar } from "./ExportBar";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(),
  open: vi.fn(),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("export", () => {
  it("n'écrit rien si le dialogue d'enregistrement est fermé", async () => {
    vi.mocked(save).mockResolvedValue(null);
    const onExport = vi.fn();
    render(<ExportBar stem="catalogue-gtm" scope="Périmètre affiché." onExport={onExport} />);
    fireEvent.click(screen.getByRole("button", { name: "Exporter" }));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "CSV" }));
    });
    expect(onExport).not.toHaveBeenCalled();
  });

  it("enregistre le format choisi", async () => {
    vi.mocked(save).mockResolvedValue("/tmp/catalogue-gtm.csv");
    const onExport = vi.fn().mockResolvedValue({
      rows: 2,
      scope: "Périmètre : produits actifs. Stock lu au moment de l'export.",
    });
    render(<ExportBar stem="catalogue-gtm" scope="Périmètre affiché." onExport={onExport} />);
    fireEvent.click(screen.getByRole("button", { name: "Exporter" }));
    fireEvent.click(screen.getByRole("button", { name: "CSV" }));
    expect(await screen.findByRole("status")).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("2 lignes");
    expect(screen.getByRole("status").textContent).toContain("produits actifs");
    expect(onExport).toHaveBeenCalledWith("/tmp/catalogue-gtm.csv", "csv");
  });

  it("bloque une période incohérente avant le dialogue", async () => {
    const onExport = vi.fn();
    render(
      <ExportBar
        stem="historique-gtm"
        scope="Période."
        beforeSave={() => "La date de fin précède la date de début."}
        onExport={onExport}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Exporter" }));
    fireEvent.click(screen.getByRole("button", { name: "PDF" }));
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(save).not.toHaveBeenCalled();
    expect(onExport).not.toHaveBeenCalled();
  });
});
