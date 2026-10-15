import type { ProductInput } from "./types";

export const NAME_MAX = 120;
export const REFERENCE_MAX = 64;
export const DESCRIPTION_MAX = 2000;

export type FieldErrors = Partial<Record<"name" | "reference" | "typeId" | "description", string>>;

export function validateProduct(input: ProductInput): FieldErrors {
  const errors: FieldErrors = {};
  const name = input.name.trim();
  const reference = input.reference.trim();
  const description = input.description.trim();

  if (name.length === 0) {
    errors.name = "Indiquez le nom du produit.";
  } else if ([...name].length > NAME_MAX) {
    errors.name = `Le nom dépasse ${NAME_MAX} caractères.`;
  }

  if ([...reference].length > REFERENCE_MAX) {
    errors.reference = `La référence dépasse ${REFERENCE_MAX} caractères.`;
  }

  if (input.typeId.trim().length === 0) {
    errors.typeId = "Choisissez un type de produit.";
  }

  if ([...description].length > DESCRIPTION_MAX) {
    errors.description = "La description dépasse 2 000 caractères.";
  }

  return errors;
}

export function validateLabel(value: string, emptyMessage: string): string | null {
  const name = value.trim();
  if (name.length === 0) {
    return emptyMessage;
  }
  if ([...name].length > NAME_MAX) {
    return `Le nom dépasse ${NAME_MAX} caractères.`;
  }
  return null;
}
