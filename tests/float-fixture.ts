import type { Page } from "@playwright/test";
import type { FloatState, QuotaSnapshot } from "../src/features/floating/types";

export const fixtureTime = "2026-09-15T12:34:30Z";
export function floatFixture(background: string): {
  state: FloatState;
  snapshot: QuotaSnapshot;
} {
  const resetTs = Date.parse(fixtureTime) / 1000 + 3660;
  return {
    state: {
      width: 290,
      height: 430,
      background,
      preferences: {
        alpha: 82,
        onTop: true,
        rounded: true,
        theme: "dark",
        animations: false,
        bgDim: 55,
        bgBlur: 8,
        glassBlur: 16,
        show: { "#usage": true },
        usagePeriod: "30d",
        usageHarnesses: {},
        resetModes: {},
        cardOrder: [],
      },
    },
    snapshot: {
      state: "cached",
      fetchedAt: fixtureTime,
      error: "",
      results: [
        {
          accountId: "alpha",
          provider: "claude",
          title: "Claude",
          email: "alpha@example.test",
          plan: "Claude Max 20x",
          planDetail: "rate_limit_tier=20x",
          subEnd: "2026-10-01T00:00:00Z",
          subStatus: "known",
          ok: true,
          error: "",
          notice: "",
          retryAt: 0,
          pending: "",
          windows: [
            {
              name: "5h quota",
              remainingPercent: 75,
              resetTs,
              reset: "",
              text: null,
            },
            {
              name: "Week quota",
              remainingPercent: 28,
              resetTs: resetTs + 86400,
              reset: "",
              text: null,
            },
          ],
          harnesses: [{ key: "claude_code", label: "Claude Code" }],
          usage: [
            {
              source: "local",
              label: "Claude Code · 近 30 天",
              period: "30d",
              harness: "claude_code",
              harnessLabel: "Claude Code",
              totalTokens: 155,
              used: null,
              total: null,
              unit: "",
              breakdown: {
                input: 100,
                cached: 10,
                cacheWrite: 5,
                output: 40,
                reasoning: null,
              },
              models: [
                {
                  name: "claude-fixture",
                  totalTokens: 155,
                  breakdown: {
                    input: 100,
                    cached: 10,
                    cacheWrite: 5,
                    output: 40,
                    reasoning: null,
                  },
                },
              ],
            },
          ],
        },
        {
          accountId: "beta",
          provider: "openai",
          title: "OpenAI",
          email: "beta@example.test",
          plan: "OpenAI (Pro 5x)",
          planDetail: "pro_lite",
          subEnd: "",
          subStatus: "",
          ok: true,
          error: "",
          notice: "",
          retryAt: 0,
          pending: "",
          windows: [
            {
              name: "Week quota",
              remainingPercent: 9,
              resetTs: resetTs + 172800,
              reset: "",
              text: null,
            },
            {
              name: "重置次数",
              remainingPercent: null,
              resetTs: null,
              reset: "",
              text: "剩余 2 次",
            },
          ],
          harnesses: [],
          usage: [],
        },
      ],
    },
  };
}
export async function installFloatMock(
  page: Page,
  fixture: ReturnType<typeof floatFixture>,
) {
  await page.clock.setFixedTime(new Date(fixtureTime));
  await page.addInitScript(({ state, snapshot }) => {
    const calls: string[] = [];
    const saved = { state, snapshot, calls, failRefresh: false };
    Object.assign(window, {
      isTauri: true,
      __floatFixture: saved,
      __TAURI_INTERNALS__: {
        metadata: {
          currentWindow: { label: "float" },
          currentWebview: { label: "float" },
        },
        transformCallback: () => 1,
        unregisterCallback: () => {},
        invoke: async (command: string, args: Record<string, unknown> = {}) => {
          calls.push(command);
          if (command === "get_float_state")
            return structuredClone(saved.state);
          if (
            command === "get_quota_snapshot" ||
            command === "refresh_quotas"
          ) {
            if (command === "refresh_quotas" && saved.failRefresh)
              throw { code: "network", message: "合成网络错误" };
            return structuredClone(saved.snapshot);
          }
          if (command === "save_float_preferences")
            saved.state.preferences =
              args.preferences as typeof state.preferences;
          if (command === "save_float_background")
            saved.state.background = args.background as string | null;
          return 1;
        },
      },
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => {} },
    });
  }, fixture);
}
export function originalFixture(fixture: ReturnType<typeof floatFixture>) {
  const preferences = fixture.state.preferences;
  const breakdown = (
    value: (typeof fixture.snapshot.results)[0]["usage"][0]["breakdown"],
  ) =>
    value
      ? {
          input: value.input,
          cached: value.cached,
          cache_write: value.cacheWrite,
          output: value.output,
          reasoning: value.reasoning,
        }
      : {};
  return {
    settings: {
      alpha: preferences.alpha,
      on_top: preferences.onTop,
      rounded: preferences.rounded,
      theme: preferences.theme,
      animations: preferences.animations,
      bg_dim: preferences.bgDim,
      bg_blur: preferences.bgBlur,
      glass_blur: preferences.glassBlur,
      show: preferences.show,
      usage_period: preferences.usagePeriod,
      usage_harnesses: preferences.usageHarnesses,
      reset_modes: preferences.resetModes,
      card_order: preferences.cardOrder,
      platform: "win32",
    },
    background: fixture.state.background,
    payload: {
      snapshot: { state: fixture.snapshot.state, fetched_at: fixtureTime },
      results: fixture.snapshot.results.map((account) => ({
        provider: account.provider,
        identity: account.accountId,
        title: account.title,
        email: account.email,
        plan: account.plan,
        plan_detail: account.planDetail,
        sub_end: account.subEnd,
        sub_status: account.subStatus,
        ok: account.ok,
        error: account.error,
        notice: account.notice,
        retry_at: account.retryAt,
        harnesses: account.harnesses,
        windows: account.windows.map((row) => ({
          name: row.name,
          remaining_percent: row.remainingPercent,
          reset_ts: row.resetTs,
          reset: row.reset,
          text: row.text,
        })),
        usage: account.usage.map((row) => ({
          source: row.source,
          label: row.label,
          period: row.period,
          harness: row.harness,
          harness_label: row.harnessLabel,
          total_tokens: row.totalTokens,
          used: row.used,
          total: row.total,
          unit: row.unit,
          breakdown: breakdown(row.breakdown),
          models: row.models.map((model) => ({
            name: model.name,
            total_tokens: model.totalTokens,
            ...breakdown(model.breakdown),
          })),
        })),
      })),
    },
  };
}
