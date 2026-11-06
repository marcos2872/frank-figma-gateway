export type ErrorCode =
  | "unsupportedPlatform"
  | "dataDirNotWritable"
  | "diskSpace"
  | "network"
  | "timeout"
  | "cancelled"
  | "manifestSignature"
  | "manifestSchema"
  | "appTooOld"
  | "componentNotAllowed"
  | "hashMismatch"
  | "sizeMismatch"
  | "rangeInvalid"
  | "zipUnsafe"
  | "extraction"
  | "versionProbe"
  | "versionMismatch"
  | "activation"
  | "rollback"
  | "portInUse"
  | "processSpawn"
  | "processExited"
  | "healthTimeout"
  | "providerAuth"
  | "providerForbidden"
  | "providerModelNotFound"
  | "providerRateLimited"
  | "providerBadRequest"
  | "providerUnavailable"
  | "copilotAuthPending"
  | "copilotAuthExpired"
  | "copilotPlanMissing"
  | "copilotDatabase"
  | "caGeneration"
  | "caTrust"
  | "caRemove"
  | "envRead"
  | "envWrite"
  | "envRestore"
  | "figmaNotFound"
  | "figmaRunning"
  | "figmaRestart"
  | "proxyTls"
  | "proxyRewrite"
  | "proxyUpstream"
  | "configInvalid"
  | "configApply"
  | "stateCorrupt"
  | "stateSchema"
  | "stepOrder"
  | "stepNotStarted"
  | "stepCannotSkip"
  | "internal";

export interface UiError {
  code: ErrorCode;
  message: string;
  detail?: string | null;
  retryable: boolean;
  action?: string | null;
}

export type StepId =
  | "welcome"
  | "runtime"
  | "provider"
  | "models"
  | "figma"
  | "finish";

export type StepStatus =
  | "pending"
  | "inProgress"
  | "done"
  | "skipped"
  | "failed";

export type SetupStatus =
  | "notStarted"
  | "inProgress"
  | "completed"
  | "needsAttention";

export interface StepState {
  id: StepId;
  status: StepStatus;
  stale: boolean;
  completedAt?: string | null;
  skippedReason?: string | null;
  lastError?: UiError | null;
}

export interface RuntimeState {
  litellmVersion?: string | null;
  activeLitellm?: string | null;
  pgsqlVersion?: string | null;
  activePgsql?: string | null;
}

export type ProviderKind =
  | "openRouter"
  | "azureAi"
  | "anthropic"
  | "openAi"
  | "gemini"
  | "gitHubCopilot"
  | "openAiCompatible";

export type TestStatus = "notTested" | "ok" | "failed" | "skipped";

export interface ProviderSelection {
  profileId: string;
  kind: ProviderKind;
  label: string;
  apiBase?: string | null;
  apiVersion?: string | null;
  deployment?: string | null;
  secretRef?: string | null;
  testStatus: TestStatus;
  testedAt?: string | null;
}

export interface ModelRoute {
  publicName: string;
  providerModel: string;
  credentialName?: string | null;
  apiKeyEnv?: string | null;
  apiBase?: string | null;
  apiVersion?: string | null;
  extra: Record<string, string>;
}

export interface ModelSettings {
  routes: ModelRoute[];
  fallbacks: Record<string, string[]>;
  appliedAt?: string | null;
  configRevision?: string | null;
}

export type AuthStatus =
  | "notStarted"
  | "waitingForUser"
  | "connected"
  | "expired"
  | "failed";

export interface CopilotState {
  enabled: boolean;
  postgresReady: boolean;
  databaseName: string;
  credentialName: string;
  connected: boolean;
  githubLogin?: string | null;
  authStatus: AuthStatus;
  lastError?: UiError | null;
}

export interface ServicePorts {
  litellm?: number | null;
  proxy?: number | null;
  postgres?: number | null;
}

export interface FigmaState {
  detected: boolean;
  installPath?: string | null;
  version?: string | null;
  running: boolean;
  integrationActive: boolean;
  caThumbprint?: string | null;
  envInstalled: boolean;
  proxyPort?: number | null;
  lastValidatedAt?: string | null;
  skipped: boolean;
}

export interface OnboardingState {
  offlineMode: boolean;
  skippedFigma: boolean;
  startWithWindows: boolean;
  completedAt?: string | null;
}

export interface SetupState {
  schemaVersion: number;
  revision: number;
  status: SetupStatus;
  currentStep: StepId;
  steps: StepState[];
  runtime: RuntimeState;
  provider?: ProviderSelection | null;
  copilot: CopilotState;
  models: ModelSettings;
  services: ServicePorts;
  figma: FigmaState;
  onboarding: OnboardingState;
  updatedAt?: string | null;
}

export type StepOutcome =
  | { kind: "done" }
  | { kind: "skipped"; reason: string };

export type ResetScope = "wizard" | "figma" | "all";

export type ServiceState =
  | "stopped"
  | "starting"
  | "running"
  | "degraded"
  | "failed"
  | "stopping";

export type HealthState = "unknown" | "ok" | "timeout" | "error";

export interface ServiceStatus {
  name: string;
  state: ServiceState;
  detail?: string | null;
  port?: number | null;
  health: HealthState;
  restartCount: number;
  lastError?: UiError | null;
}

export interface AppInfo {
  name: string;
  version: string;
  models: string[];
  proxyPrefix: string;
}

export interface SetupResult {
  ok: boolean;
  message: string;
}

export type ComponentId = "litellm" | "pgsql";

export interface ComponentManifest {
  version: string;
  url: string;
  sha256: string;
  size: number;
  installedSize?: number | null;
  entrypoint?: string | null;
  versionArgs?: string[];
  minAppVersion?: string | null;
}

export interface RuntimeManifest {
  schemaVersion: number;
  generatedAt?: string | null;
  minAppVersion?: string | null;
  litellm: ComponentManifest;
  pgsql?: ComponentManifest | null;
}

export interface ActiveComponent {
  version: string;
  path: string;
}

export interface ActiveRuntime {
  litellm?: ActiveComponent | null;
  pgsql?: ActiveComponent | null;
}

export interface Installation {
  component: ComponentId;
  version: string;
  path: string;
  previous?: ActiveComponent | null;
}

export type DownloadPhase = "downloading" | "verifying" | "complete";

export interface DownloadProgress {
  component: ComponentId;
  downloaded: number;
  total: number;
  attempt: number;
  phase: DownloadPhase;
}

export type FieldKind = "text" | "password" | "url" | "select";

export interface ProviderField {
  id: string;
  label: string;
  kind: FieldKind;
  required: boolean;
  secret: boolean;
  placeholder?: string | null;
}

export interface ProviderDescriptor {
  kind: ProviderKind;
  label: string;
  description: string;
  requiresPostgres: boolean;
  fields: ProviderField[];
  recommendedModel?: string | null;
}

export interface ProviderProfileInput {
  kind: ProviderKind;
  label: string;
  profileId?: string | null;
  apiKey?: string | null;
  apiBase?: string | null;
  apiVersion?: string | null;
  deployment?: string | null;
}

export interface ProviderTestResult {
  ok: boolean;
  status: number;
  message: string;
  latencyMs: number;
}

export interface CaStatus {
  generated: boolean;
  installed: boolean;
  thumbprint?: string | null;
  certPath?: string | null;
}

export interface ProxyStatus {
  running: boolean;
  port: number;
  connections: number;
  aiRequests: number;
  failures: number;
  firstAiRequestAt?: string | null;
  lastError?: string | null;
}

export interface ProxySelfTest {
  ok: boolean;
  status: number;
  message: string;
}

export interface FigmaInfo {
  detected: boolean;
  path?: string | null;
  version?: string | null;
  running: boolean;
}

export interface IntegrationView {
  status: "inactive" | "active" | "partial" | "removing";
  caInstalled: boolean;
  caThumbprint?: string | null;
  proxyRunning: boolean;
  proxyPort?: number | null;
  envInstalled: boolean;
  figma: FigmaInfo;
}

export interface IntegrationCheck {
  id: string;
  ok: boolean;
  blocking: boolean;
  message: string;
}

export interface IntegrationValidation {
  ok: boolean;
  checks: IntegrationCheck[];
}

export interface PostgresStatus {
  initialized: boolean;
  running: boolean;
  port?: number | null;
  dataDir: string;
}

export interface CopilotStatus {
  copilot: CopilotState;
  postgres: PostgresStatus;
}

export type CheckId =
  | "platform"
  | "dataDir"
  | "disk"
  | "internet"
  | "figma"
  | "webView2"
  | "ports"
  | "existingProxy"
  | "admin"
  | "clock";

export type CheckStatus = "pass" | "warn" | "fail" | "unknown";

export interface EnvironmentCheck {
  id: CheckId;
  status: CheckStatus;
  message: string;
  detail?: string | null;
}

export interface FigmaDetection {
  found: boolean;
  path?: string | null;
  version?: string | null;
  running: boolean;
}

export interface ProxyEnvironment {
  httpsProxy?: string | null;
  httpProxy?: string | null;
  noProxy?: string | null;
}

export interface EnvironmentReport {
  os: string;
  arch: string;
  supported: boolean;
  dataDir: string;
  freeDiskBytes: number;
  requiredDiskBytes: number;
  checks: EnvironmentCheck[];
  figma: FigmaDetection;
  existingProxy: ProxyEnvironment;
  ports: Array<{ port: number; available: boolean; detail?: string | null }>;
  blocking: boolean;
  warnings: string[];
}

export type LogStream = "stdout" | "stderr";

export interface LogLine {
  stream: LogStream;
  line: string;
}

export type ServiceName = "postgres" | "litellm" | "proxy";
