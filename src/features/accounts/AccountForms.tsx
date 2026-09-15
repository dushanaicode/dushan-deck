import { useState } from "react";
import type { FormEvent } from "react";
import type {
  Provider,
  ProviderInfo,
  ImportFormat,
  Snapshot,
} from "../../lib/contracts";
import { api, errorMessage } from "../../lib/deck";
import { Modal } from "../../components/Modal";

export function VaultForm({
  configured,
  onDone,
  onClose,
}: {
  configured: boolean;
  onDone: () => void;
  onClose: () => void;
}) {
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!configured && password !== confirm) {
      setError("两次输入的口令不一致");
      return;
    }
    setBusy(true);
    setError("");
    const secret = password;
    setPassword("");
    setConfirm("");
    try {
      await api.unlock(secret);
      onDone();
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal
      title={configured ? "解锁本地凭据库" : "创建本地凭据库"}
      onClose={onClose}
    >
      <p className="muted">
        账号凭据加密保存在本机。每次启动后解锁，口令仅用于本地加密，不会发送给模型服务。
      </p>
      <form onSubmit={(event) => void submit(event)}>
        <label>
          本地口令
          <input
            autoFocus
            type="password"
            autoComplete="off"
            minLength={12}
            maxLength={1024}
            required
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
        </label>
        {!configured && (
          <>
            <label>
              再次输入口令
              <input
                type="password"
                autoComplete="off"
                minLength={12}
                required
                value={confirm}
                onChange={(event) => setConfirm(event.target.value)}
              />
            </label>
            <p className="note">请妥善保管口令，丢失后无法解密已保存的凭据。</p>
          </>
        )}
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
        <button className="primary" disabled={busy}>
          {busy ? "正在解锁…" : configured ? "解锁" : "创建并解锁"}
        </button>
      </form>
    </Modal>
  );
}

export function ImportForm({
  info,
  onDone,
  onClose,
}: {
  info: ProviderInfo;
  onDone: (message: string) => void;
  onClose: () => void;
}) {
  const [label, setLabel] = useState("");
  const provider = info.id;
  const [format, setFormat] = useState<ImportFormat>(info.importFormats[0]);
  const [content, setContent] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError("");
    const secret = content;
    setContent("");
    try {
      const result = await api.import({
        provider,
        label,
        format,
        content: secret,
      });
      onDone(
        result.outcome === "created"
          ? "账号已加密保存，服务身份尚未验证"
          : "已更新原有会话，其他账号保持独立",
      );
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal title={`导入 ${info.name} 账号`} onClose={onClose}>
      <form onSubmit={(event) => void submit(event)}>
        <label>
          账号名称
          <input
            autoFocus
            placeholder="例如：个人开发账号"
            required
            maxLength={80}
            value={label}
            onChange={(event) => setLabel(event.target.value)}
          />
        </label>
        <label>
          接入方式
          <select
            value={format}
            onChange={(event) => {
              setFormat(event.target.value as typeof format);
              setContent("");
            }}
          >
            {info.importFormats.map((format) => (
              <option key={format} value={format}>
                {
                  {
                    api_key: "API Key",
                    claude_code: "Claude Code JSON",
                    codex: "Codex auth.json",
                    quota_json: "Quota 账号 JSON",
                  }[format]
                }
              </option>
            ))}
          </select>
        </label>
        <label>
          {format === "api_key" ? "API Key" : "完整 JSON 凭据组"}
          {format === "api_key" ? (
            <input
              type="password"
              autoComplete="off"
              placeholder="粘贴 API Key"
              required
              minLength={8}
              value={content}
              onChange={(event) => setContent(event.target.value)}
            />
          ) : (
            <textarea
              autoComplete="off"
              spellCheck={false}
              placeholder="粘贴同一会话的完整凭据文件内容"
              rows={7}
              required
              maxLength={262144}
              value={content}
              onChange={(event) => setContent(event.target.value)}
            />
          )}
        </label>
        <p className="note">
          仅导入你选择的内容。保存不代表登录态、额度或模型调用能力已验证；不同来源不会自动合并账号。
          {format !== "api_key" &&
            "导入的 OAuth 会话会在后台续期；客户端自动回写尚未接通，共用同一登录会话时请留意原客户端的凭据更新。"}
        </p>
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
        <button className="primary" disabled={busy}>
          {busy ? "正在加密保存…" : "加密保存账号"}
        </button>
      </form>
    </Modal>
  );
}

export function ConnectionForm({
  provider,
  snapshot,
  onDone,
  onClose,
}: {
  provider: Provider;
  snapshot: Snapshot;
  onDone: () => void;
  onClose: () => void;
}) {
  const credentials = snapshot.credentials.filter((credential) =>
    snapshot.accounts.some(
      (account) =>
        account.id === credential.accountId && account.provider === provider,
    ),
  );
  const [credentialId, setCredentialId] = useState(credentials[0].id);
  const [label, setLabel] = useState("");
  const [model, setModel] = useState("");
  const [baseUrl, setBaseUrl] = useState(
    provider === "claude"
      ? "https://api.anthropic.com"
      : "https://api.openai.com/v1",
  );
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError("");
    try {
      await api.createConnection({
        credentialSetId: credentialId,
        label,
        model,
        baseUrl,
      });
      onDone();
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal title="添加模型连接" onClose={onClose}>
      <form onSubmit={(event) => void submit(event)}>
        <label>
          连接名称
          <input
            autoFocus
            required
            maxLength={80}
            value={label}
            onChange={(event) => setLabel(event.target.value)}
          />
        </label>
        <label>
          账号与凭据组
          <select
            value={credentialId}
            onChange={(event) => setCredentialId(event.target.value)}
          >
            {credentials.map((credential) => (
              <option key={credential.id} value={credential.id}>
                {
                  snapshot.accounts.find(
                    (account) => account.id === credential.accountId,
                  )!.label
                }{" "}
                · {credential.kind === "oauth" ? "OAuth" : "API Key"} · v
                {credential.version}
              </option>
            ))}
          </select>
        </label>
        <label>
          模型标识
          <input
            required
            placeholder="填写服务提供方声明的 model ID"
            maxLength={160}
            value={model}
            onChange={(event) => setModel(event.target.value)}
          />
        </label>
        <label>
          服务地址
          <input
            type="url"
            required
            value={baseUrl}
            onChange={(event) => setBaseUrl(event.target.value)}
          />
        </label>
        <p className="note">保存连接配置后仍需单独验证授权与调用能力。</p>
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
        <button className="primary" disabled={busy}>
          {busy ? "正在保存…" : "保存连接"}
        </button>
      </form>
    </Modal>
  );
}
