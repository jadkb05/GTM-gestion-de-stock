export const SCALE = 1000;
export const MAX_MILLI = 1_000_000_000_000;

export function parseQuantity(raw: string): { milli: number } | { error: string } {
  return parseScaled(raw, false);
}

export function parseNonNegative(raw: string): { milli: number } | { error: string } {
  return parseScaled(raw, true);
}

function parseScaled(raw: string, allowZero: boolean): { milli: number } | { error: string } {
  const trimmed = raw.trim();
  if (trimmed.length === 0) {
    return { error: "Indiquez une quantité." };
  }
  if (trimmed.startsWith("-")) {
    return {
      error: allowZero
        ? "La quantité doit être positive ou nulle."
        : "La quantité doit être strictement positive.",
    };
  }
  if (trimmed.startsWith("+") || /\s/.test(trimmed)) {
    return { error: "La quantité n'est pas un nombre valide." };
  }
  const comma = trimmed.includes(",");
  const dot = trimmed.includes(".");
  if (comma && dot) {
    return { error: "La quantité n'est pas un nombre valide." };
  }
  const normalized = comma ? trimmed.replace(",", ".") : trimmed;
  if ([...normalized].filter((char) => char === ".").length > 1) {
    return { error: "La quantité n'est pas un nombre valide." };
  }
  const [whole = "", frac = ""] = normalized.split(".");
  if (whole.length === 0 && frac.length === 0) {
    return { error: "La quantité n'est pas un nombre valide." };
  }
  if (!/^\d*$/.test(whole) || !/^\d*$/.test(frac)) {
    return { error: "La quantité n'est pas un nombre valide." };
  }
  if (frac.length > 3) {
    return { error: "La quantité dépasse 3 décimales." };
  }
  const wholeValue = whole.length === 0 ? 0 : Number(whole);
  const fracValue = frac.length === 0 ? 0 : Number(frac.padEnd(3, "0"));
  if (!Number.isSafeInteger(wholeValue) || !Number.isSafeInteger(fracValue)) {
    return { error: "La quantité dépasse la limite autorisée." };
  }
  const milli = wholeValue * SCALE + fracValue;
  if (!Number.isSafeInteger(milli) || milli > MAX_MILLI) {
    return { error: "La quantité dépasse la limite autorisée." };
  }
  if (milli === 0 && !allowZero) {
    return { error: "La quantité doit être strictement positive." };
  }
  return { milli };
}

export function formatQuantity(milli: number): string {
  const negative = milli < 0;
  const absolute = Math.abs(milli);
  const whole = Math.trunc(absolute / SCALE);
  const frac = absolute % SCALE;
  let body = String(whole);
  if (frac !== 0) {
    body = `${whole}.${String(frac).padStart(3, "0").replace(/0+$/, "")}`;
  }
  return negative ? `-${body}` : body;
}

export function displayQuantity(quantity: string): string {
  return quantity.replace(".", ",");
}

export function movementLabel(direction: "in" | "out", source?: string | null): string {
  if (source === "inventory") {
    return "Correction d'inventaire";
  }
  return direction === "in" ? "Entrée" : "Sortie";
}

export function optionalText(value: string, max: number, message: string): string | null {
  const trimmed = value.trim();
  if ([...trimmed].length > max) {
    return message;
  }
  return null;
}
