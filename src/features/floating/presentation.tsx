import type {
  FloatPreferences,
  QuotaAccount,
  QuotaWindow,
  TokenBreakdown,
  UsageRow,
  UsagePeriod,
} from "./types";

export const accountKey = (account: QuotaAccount) =>
  `${account.provider}:${account.accountId}`;
export function countdown(timestamp: number, now: number) {
  const seconds = Math.floor(timestamp - now / 1000);
  if (seconds <= 0) return "待重置";
  const days = Math.floor(seconds / 86400),
    hours = Math.floor((seconds % 86400) / 3600),
    minutes = Math.floor((seconds % 3600) / 60);
  const pad = (n: number) => String(n).padStart(2, "0");
  if (days) return `${days}d${pad(hours)}h${pad(minutes)}m`;
  if (hours) return `${hours}h${pad(minutes)}m`;
  return minutes ? `${minutes}m` : "<1m";
}
export function resetAt(timestamp: number) {
  const date = new Date(timestamp * 1000),
    pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}
export function formatUsage(value: number) {
  if (Math.abs(value) >= 1e9)
    return (
      (value / 1e9).toFixed(value >= 1e10 ? 1 : 2).replace(/\.?0+$/, "") + "B"
    );
  if (Math.abs(value) >= 1e6)
    return (
      (value / 1e6).toFixed(value >= 1e7 ? 1 : 2).replace(/\.?0+$/, "") + "M"
    );
  if (Math.abs(value) >= 1e3)
    return (
      (value / 1e3).toFixed(value >= 1e4 ? 1 : 2).replace(/\.?0+$/, "") + "K"
    );
  return Number.isInteger(value)
    ? value.toLocaleString()
    : value.toLocaleString([], { maximumFractionDigits: 2 });
}
function UsageStats({ value }: { value: TokenBreakdown | null }) {
  if (!value) return null;
  const values: [string, number | null, boolean][] = [
    ["输入", value.input, true],
    ["缓存读", value.cached, (value.cached ?? 0) > 0],
    ["缓存写", value.cacheWrite, (value.cacheWrite ?? 0) > 0],
    ["输出", value.output, true],
    ["推理", value.reasoning, (value.reasoning ?? 0) > 0],
  ];
  const visible = values.filter(([, amount, show]) => show && amount !== null);
  return visible.length ? (
    <div className="usage-stats">
      {visible.map(([label, amount]) => (
        <span
          className="usage-stat"
          title={amount!.toLocaleString()}
          key={label}
        >
          {label} {formatUsage(amount!)}
        </span>
      ))}
    </div>
  ) : null;
}
export function QuotaRow({
  row,
  account,
  settings,
  now,
  change,
}: {
  row: QuotaWindow;
  account: string;
  settings: FloatPreferences;
  now: number;
  change: (settings: FloatPreferences) => void;
}) {
  if (row.text !== null || row.remainingPercent === null)
    return (
      <div className="row">
        <div className="top">
          <span className="lbl">{row.name}</span>
          <span className="rst" />
          <span className="txt">{row.text ?? "暂无数据"}</span>
        </div>
      </div>
    );
  const percent = Math.max(
    0,
    Math.min(100, Math.round(row.remainingPercent * 10) / 10),
  );
  const color = percent >= 40 ? "g" : percent >= 15 ? "y" : "r";
  const key = JSON.stringify([account, row.name]);
  const absolute = settings.resetModes[key] === true;
  return (
    <div className="row">
      <div className="top">
        <span className="lbl">{row.name}</span>
        {row.resetTs !== null ? (
          <button
            type="button"
            className="rst rst-toggle"
            data-reset-ts={row.resetTs}
            data-reset-key={key}
            aria-pressed={absolute}
            title={`额度重置时间：${resetAt(row.resetTs)}；点击切换${absolute ? "倒计时" : "重置时间"}`}
            onClick={() =>
              change({
                ...settings,
                resetModes: { ...settings.resetModes, [key]: !absolute },
              })
            }
          >
            {absolute
              ? `重置于 ${resetAt(row.resetTs)}`
              : countdown(row.resetTs, now)}
          </button>
        ) : (
          <span className="rst">{row.reset}</span>
        )}
        <span className={`pct ${color}`}>{`${percent}%`}</span>
      </div>
      <div className="bar">
        <i
          data-bar={`${account}|${row.name}`}
          className={color}
          style={{ width: `${percent}%` }}
        />
      </div>
    </div>
  );
}
function aggregate(rows: UsageRow[], period: UsagePeriod): UsageRow {
  const sum = (key: keyof TokenBreakdown) =>
    rows.reduce((total, row) => total + (row.breakdown?.[key] ?? 0), 0);
  return {
    source: "local",
    label: `本机合计 · ${period === "all" ? "累计" : `近 ${period.replace("d", "")} 天`}`,
    period,
    harness: "all",
    harnessLabel: "全部客户端",
    totalTokens: rows.reduce((total, row) => total + (row.totalTokens ?? 0), 0),
    used: null,
    total: null,
    unit: "",
    models: [],
    breakdown: {
      input: sum("input"),
      cached: sum("cached"),
      cacheWrite: sum("cacheWrite"),
      output: sum("output"),
      reasoning: sum("reasoning"),
    },
  };
}
export function UsageBlock({
  account,
  settings,
  change,
}: {
  account: QuotaAccount;
  settings: FloatPreferences;
  change: (settings: FloatPreferences) => void;
}) {
  const { usage: rows, harnesses } = account;
  if (!settings.show["#usage"] || (!rows.length && !harnesses.length))
    return null;
  const key = accountKey(account),
    period = settings.usagePeriod,
    hasRemote = rows.some((row) => row.source === "remote");
  const selected = settings.usageHarnesses[key] ?? "all";
  const harness =
    selected === "all" ||
    (selected === "remote" && hasRemote) ||
    harnesses.some((item) => item.key === selected)
      ? selected
      : "all";
  const local = rows.filter(
      (row) => row.period === period && row.source === "local",
    ),
    remote = rows.filter(
      (row) => row.period === period && row.source === "remote",
    );
  const visible =
    harness === "all"
      ? [
          ...(local.length === 1
            ? local
            : local.length
              ? [aggregate(local, period)]
              : []),
          ...remote,
        ]
      : harness === "remote"
        ? remote
        : local.filter((row) => row.harness === harness);
  const periods: [UsagePeriod, string][] = [
    ["1d", "近 1 天"],
    ["3d", "近 3 天"],
    ["7d", "近 7 天"],
    ["30d", "近 30 天"],
    ["all", "累计"],
  ];
  return (
    <div className="usage">
      <div className="usage-title">用量信息</div>
      <div className="usage-filter">
        <select
          aria-label="时间范围"
          title="时间范围"
          value={period}
          onChange={(event) =>
            change({
              ...settings,
              usagePeriod: event.target.value as UsagePeriod,
            })
          }
        >
          {periods.map(([value, label]) => (
            <option value={value} key={value}>
              {label}
            </option>
          ))}
        </select>
        <select
          aria-label="客户端"
          title="客户端"
          value={harness}
          onChange={(event) =>
            change({
              ...settings,
              usageHarnesses: {
                ...settings.usageHarnesses,
                [key]: event.target.value,
              },
            })
          }
        >
          <option value="all">全部客户端</option>
          {hasRemote && <option value="remote">远端</option>}
          {harnesses.map((item) => (
            <option key={item.key} value={item.key}>
              {item.label}
            </option>
          ))}
        </select>
      </div>
      {!visible.length && <div className="txt">暂无用量</div>}
      {visible.map((row) => (
        <div
          className="usage-entry"
          key={`${row.source}:${row.harness}:${row.label}`}
          style={{ display: "contents" }}
        >
          <div className="usage-line">
            <span className="src">
              {row.source === "local" ? "本机" : "远端"}
            </span>
            <span className="ulbl">{row.label}</span>
            <span className="uval">
              {row.totalTokens !== null
                ? `${formatUsage(row.totalTokens)} Token`
                : row.used !== null && row.total !== null
                  ? `${formatUsage(row.used)} / ${formatUsage(row.total)}${row.unit ? ` ${row.unit}` : ""}`
                  : ""}
            </span>
          </div>
          <UsageStats value={row.breakdown} />
          {row.models.map((model) => (
            <div className="usage-model" key={model.name}>
              <div className="usage-model-main">
                <span className="ulbl">{model.name || "未标记模型"}</span>
                <span className="uval">
                  {model.totalTokens === null
                    ? ""
                    : `${formatUsage(model.totalTokens)} Token`}
                </span>
              </div>
              <UsageStats value={model.breakdown} />
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
