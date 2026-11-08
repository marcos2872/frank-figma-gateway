import type { UiError } from "./contracts";

const FALLBACK = "Falha inesperada. Tente novamente.";

/**
 * Normaliza qualquer erro para uma mensagem legível.
 *
 * O `invoke` do Tauri rejeita com o objeto `UiError` serializado, e não com um
 * `Error`; sem essa normalização a UI exibe apenas `[object Object]`.
 */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message || FALLBACK;
  }
  if (isUiError(error)) {
    return error.message || error.detail || FALLBACK;
  }
  if (error == null) {
    return FALLBACK;
  }
  return String(error);
}

/** Detecta o contrato `UiError` devolvido pelos comandos Tauri. */
export function isUiError(value: unknown): value is UiError {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const candidate = value as { code?: unknown; message?: unknown };
  return (
    typeof candidate.code === "string" && typeof candidate.message === "string"
  );
}
