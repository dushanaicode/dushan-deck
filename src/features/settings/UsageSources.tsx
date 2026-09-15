import { useCallback, useEffect, useState } from "react";
import type { FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Snapshot } from "../../lib/contracts";
import { errorMessage } from "../../lib/deck";

interface Source {
  id: string;
  accountId: string;
  harness: string;
  path: string;
  since: number;
}
const harnesses = {
  codex: "Codex",
  claude_code: "Claude Code",
  opencode: "OpenCode",
  omp: "OMP",
  kimi_code: "Kimi Code CLI",
  grok_cli: "Grok CLI",
};
export function UsageSources({ snapshot }: { snapshot: Snapshot }) {
  const [sources, setSources] = useState<Source[]>([]),
    [error, setError] = useState("");
  const [accountId, setAccountId] = useState(snapshot.accounts[0]?.id ?? ""),
    [harness, setHarness] = useState("codex"),
    [path, setPath] = useState("");
  const [since, setSince] = useState(""),
    [busy, setBusy] = useState(false);
  const reload = useCallback(async () => {
    setSources(await invoke<Source[]>("get_usage_sources"));
  }, []);
  useEffect(() => {
    void reload().catch((error: unknown) => setError(errorMessage(error)));
  }, [reload]);
  async function save(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError("");
    try {
      await invoke("save_usage_source", {
        source: {
          id: "",
          accountId,
          harness,
          path,
          since: Math.floor(new Date(since).getTime() / 1000),
        },
      });
      setPath("");
      await reload();
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }
  async function remove(id: string) {
    setBusy(true);
    try {
      await invoke("remove_usage_source", { id });
      await reload();
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="panel">
      <div className="section-heading">
        <div>
          <h2>本机用量来源</h2>
          <p>只读统计客户端日志；切号时间决定账号归属</p>
        </div>
      </div>
      <p className="note" style={{ marginTop: 18 }}>
        选择账号、日志位置和该账号开始使用此客户端的时间。更早或无法确认归属的记录会单独报告。切换账号时添加新的起点，保留历史起点。
      </p>
      {sources.map((source) => (
        <div className="setting-row" key={source.id}>
          <span>
            <strong>
              {
                snapshot.accounts.find(
                  (account) => account.id === source.accountId,
                )!.label
              }{" "}
              · {harnesses[source.harness as keyof typeof harnesses]}
            </strong>
            <small>
              {source.path}
              <br />从 {new Date(source.since * 1000).toLocaleString()} 起
            </small>
          </span>
          <button
            className="secondary"
            disabled={busy}
            onClick={() => void remove(source.id)}
          >
            移除来源
          </button>
        </div>
      ))}
      {snapshot.accounts.length > 0 && (
        <form onSubmit={(event) => void save(event)}>
          <label>
            所属账号
            <select
              value={accountId}
              onChange={(event) => setAccountId(event.target.value)}
            >
              {snapshot.accounts.map((account) => (
                <option value={account.id} key={account.id}>
                  {
                    snapshot.providers.find(
                      (provider) => provider.id === account.provider,
                    )!.name
                  }{" "}
                  · {account.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            客户端
            <select
              value={harness}
              onChange={(event) => setHarness(event.target.value)}
            >
              {Object.entries(harnesses).map(([value, label]) => (
                <option value={value} key={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label>
            {harness === "opencode" ? "OpenCode 数据库路径" : "客户端会话目录"}
            <input
              required
              value={path}
              onChange={(event) => setPath(event.target.value)}
              placeholder={
                harness === "opencode"
                  ? "选择 opencode.db 的绝对路径"
                  : "填写 sessions 或 projects 目录的绝对路径"
              }
            />
          </label>
          <label>
            此账号的归属起点
            <input
              type="datetime-local"
              required
              value={since}
              onChange={(event) => setSince(event.target.value)}
            />
          </label>
          <button className="secondary" disabled={busy}>
            添加用量来源
          </button>
        </form>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
