export type Provider = "claude" | "openai";
export interface ProviderInfo {
  id: Provider;
  name: string;
  description: string;
  importFormats: string[];
  quotaAvailable: boolean;
  refreshAvailable: boolean;
  clientWriteAvailable: boolean;
}
export interface Account {
  id: string;
  provider: Provider;
  label: string;
  identityStatus: "unverified";
  createdAt: number;
}
export interface CredentialSet {
  id: string;
  accountId: string;
  kind: "api_key" | "oauth";
  source: string;
  version: number;
  expiresAt: number | null;
}
export interface ModelConnection {
  id: string;
  accountId: string;
  credentialSetId: string;
  label: string;
  model: string;
  baseUrl: string;
  verified: boolean;
}
export interface Task {
  id: string;
  kind: string;
  state:
    | "queued"
    | "running"
    | "waiting_user"
    | "paused"
    | "succeeded"
    | "failed"
    | "cancelled"
    | "interrupted";
  createdAt: number;
  finishedAt: number | null;
  message: string;
}
export interface Settings {
  favoriteProviders: Provider[];
  floatEnabled: boolean;
}
export interface Snapshot {
  schemaVersion: number;
  providers: ProviderInfo[];
  accounts: Account[];
  credentials: CredentialSet[];
  connections: ModelConnection[];
  tasks: Task[];
  settings: Settings;
  vaultConfigured: boolean;
  vaultUnlocked: boolean;
}
export interface ImportRequest {
  provider: Provider;
  label: string;
  format: "api_key" | "claude_code" | "codex";
  content: string;
}
export interface ImportResult {
  accountId: string;
  credentialSetId: string;
  outcome: "created" | "updated";
}
export interface ConnectionRequest {
  credentialSetId: string;
  label: string;
  model: string;
  baseUrl: string;
}
export interface ErrorDto {
  code: string;
  message: string;
  action: string;
}
