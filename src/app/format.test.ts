import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { backupReminder, formatDateTime, formatToday, plural } from "./format";

describe("formatDateTime", () => {
  beforeAll(() => {
    vi.stubEnv("TZ", "Europe/Paris");
  });
  afterAll(() => {
    vi.unstubAllEnvs();
  });

  it("convertit l'horodatage UTC de la base en heure locale", () => {
    expect(formatDateTime("2026-10-13T21:07:23.251Z")).toBe("13/10/2026 23:07");
    expect(formatDateTime("2026-10-13T21:07:23.251Z", true)).toBe("13/10/2026 23:07:23");
  });

  it("place un mouvement proche de minuit sur le bon jour local", () => {
    expect(formatDateTime("2026-06-30T22:30:00.000Z")).toBe("01/07/2026 00:30");
    expect(formatDateTime("2026-01-15T23:30:00.000Z")).toBe("16/01/2026 00:30");
  });

  it("laisse inchangée une valeur sans fuseau", () => {
    expect(formatDateTime("2026-10-13 23:07:23")).toBe("13/10/2026 23:07");
  });

  it("conserve une valeur inattendue et signale l'absence", () => {
    expect(formatDateTime("hier")).toBe("hier");
    expect(formatDateTime(null)).toBe("—");
  });
});

describe("formatToday", () => {
  it("écrit la date du jour en français", () => {
    expect(formatToday(new Date(2026, 9, 14))).toBe("Mercredi 14 octobre 2026");
  });
});

describe("plural", () => {
  it("accorde le libellé", () => {
    expect(plural(1, "produit", "produits")).toBe("1 produit");
    expect(plural(3, "produit", "produits")).toBe("3 produits");
  });
});

describe("backupReminder", () => {
  const now = new Date("2026-10-15T10:00:00.000Z");

  it("signale l'absence de sauvegarde", () => {
    expect(backupReminder(null, now)?.tone).toBe("danger");
  });

  it("ne dit rien pour une sauvegarde récente", () => {
    expect(backupReminder("2026-10-12T10:00:00.000Z", now)).toBeNull();
  });

  it("avertit au-delà de sept jours", () => {
    const reminder = backupReminder("2026-10-01T10:00:00.000Z", now);
    expect(reminder?.tone).toBe("warn");
    expect(reminder?.text).toContain("il y a 14 jours");
  });
});
