import type { ProviderKind } from "../../lib/contracts";
import { errorMessage } from "../../lib/errors";

export { errorMessage };

export interface WizardStepProps {
  onNext: () => void;
}

/**
 * Ids que o seletor do Make envia (validados no Figma 126.11.7).
 * Espelha `FIGMA_MODEL_IDS` em `src/crates/core/src/models.rs`.
 */
export const FIGMA_MODEL_IDS = [
  "default",
  "anthropic-claude-4.6-opus",
  "anthropic-claude-4.8-opus",
  "anthropic-claude-5-opus",
  "anthropic-claude-5.5-opus",
  "anthropic-claude-4.6-sonnet",
  "anthropic-claude-5-sonnet",
  "anthropic-claude-5.5-sonnet",
  "figaro-sonnet-4.6",
  "figaro-gpt-5.6-sol",
  "figaro-gpt-5.6-terra",
  "figaro-gpt-6-astra",
  "figaro-gpt-6-luna",
  "figaro-gpt-6-sol",
  "google-gemini-3-pro",
  "google-gemini-3.1-pro",
  "google-gemini-3-flash",
  "google-gemini-3.8-flash",
  "aqua",
  "velvet",
  "goldenrod",
  "emerald",
  "tangelo",
] as const;

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const index = Math.min(
    units.length - 1,
    Math.floor(Math.log(bytes) / Math.log(1024)),
  );
  return `${(bytes / 1024 ** index).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

export function providerEnvVar(kind: ProviderKind): string | null {
  switch (kind) {
    case "openRouter":
      return "OPENROUTER_API_KEY";
    case "anthropic":
      return "ANTHROPIC_API_KEY";
    case "openAi":
      return "OPENAI_API_KEY";
    case "gemini":
      return "GEMINI_API_KEY";
    case "azureAi":
      return "AZURE_API_KEY";
    case "openAiCompatible":
      return "OPENAI_API_KEY";
    case "gitHubCopilot":
      return null;
  }
}

export function providerModelPrefix(kind: ProviderKind): string {
  switch (kind) {
    case "openRouter":
      return "openrouter/";
    case "anthropic":
      return "anthropic/";
    case "openAi":
      return "openai/";
    case "gemini":
      return "gemini/";
    case "azureAi":
      return "azure/";
    case "gitHubCopilot":
      return "github_copilot/";
    case "openAiCompatible":
      return "";
  }
}

export async function waitForLiteLlm(
  check: () => Promise<void>,
  attempts = 30,
): Promise<void> {
  let lastError: unknown = null;
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      await check();
      return;
    } catch (error) {
      lastError = error;
      await new Promise((resolve) => setTimeout(resolve, 1_000));
    }
  }
  throw new Error(
    lastError
      ? errorMessage(lastError)
      : "LiteLLM não ficou saudável a tempo.",
  );
}
