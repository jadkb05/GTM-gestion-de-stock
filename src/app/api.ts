import { invoke } from "@tauri-apps/api/core";
import type { AppStatus } from "./types";

export function fetchAppStatus(): Promise<AppStatus> {
  return invoke<AppStatus>("get_app_status");
}

export function toErrorMessage(error: unknown): string {
  if (typeof error === "string" && error.trim().length > 0) {
    return error;
  }
  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }
  return "La base locale n'a pas pu être initialisée.";
}
