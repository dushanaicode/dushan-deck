export type FloatTheme = "dark" | "ocean" | "forest" | "violet" | "paper";
export type UsagePeriod = "1d" | "3d" | "7d" | "30d" | "all";
export interface FloatPreferences {
  alpha: number;
  onTop: boolean;
  rounded: boolean;
  theme: FloatTheme;
  animations: boolean;
  bgDim: number;
  bgBlur: number;
  glassBlur: number;
  show: Record<string, boolean>;
  usagePeriod: UsagePeriod;
  usageHarnesses: Record<string, string>;
  resetModes: Record<string, boolean>;
  cardOrder: string[];
}
export interface FloatState {
  preferences: FloatPreferences;
  width: number;
  height: number;
  background: string | null;
}
export interface QuotaWindow {
  name: string;
  remainingPercent: number | null;
  resetTs: number | null;
  reset: string;
  text: string | null;
}
export interface TokenBreakdown {
  input: number | null;
  cached: number | null;
  cacheWrite: number | null;
  output: number | null;
  reasoning: number | null;
}
export interface UsageRow {
  source: "local" | "remote";
  label: string;
  period: UsagePeriod;
  harness: string;
  harnessLabel: string;
  totalTokens: number | null;
  used: number | null;
  total: number | null;
  unit: string;
  breakdown: TokenBreakdown | null;
  models: {
    name: string;
    totalTokens: number | null;
    breakdown: TokenBreakdown | null;
  }[];
}
export interface QuotaAccount {
  accountId: string;
  provider: string;
  title: string;
  email: string;
  plan: string;
  planDetail: string;
  subEnd: string;
  subStatus: string;
  windows: QuotaWindow[];
  usage: UsageRow[];
  harnesses: { key: string; label: string }[];
  ok: boolean;
  error: string;
  notice: string;
  retryAt: number;
  pending: string;
}
export interface QuotaSnapshot {
  results: QuotaAccount[];
  state: "fresh" | "cached" | "stale" | "error" | "locked";
  fetchedAt: string | null;
  error: string;
}
