import type { ProviderKind } from "../../lib/contracts";

export { errorMessage } from "../../lib/errors";

export interface WizardStepProps {
  onNext: () => void;
}

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
  throw lastError instanceof Error
    ? lastError
    : new Error("LiteLLM não ficou saudável a tempo.");
}
