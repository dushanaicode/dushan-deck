import type { Provider, Snapshot } from "../../lib/contracts";
import { Icon } from "../../components/Icon";

export function ProviderPage({
  provider,
  snapshot,
  onImport,
  onConnection,
}: {
  provider: Provider;
  snapshot: Snapshot;
  onImport: () => void;
  onConnection: () => void;
}) {
  const info = snapshot.providers.find((item) => item.id === provider)!;
  const accounts = snapshot.accounts.filter(
    (account) => account.provider === provider,
  );
  const connections = snapshot.connections.filter((connection) =>
    accounts.some((account) => account.id === connection.accountId),
  );
  return (
    <>
      <div className="page-heading">
        <div>
          <span className="eyebrow">
            MODEL SPACE / {provider.toUpperCase()}
          </span>
          <h1>
            {info.name}
            <span className={`provider-dot ${provider}`} />
          </h1>
          <p>{info.description} · 管理账号、凭据与模型连接</p>
        </div>
        <button className="primary" onClick={onImport}>
          <Icon name="plus" size={17} />
          导入账号
        </button>
      </div>
      <div className="section-tabs">
        <span className="active">模型与账号</span>
        <span>额度 · 悬浮窗</span>
        <span>本机接入 · 待接入</span>
      </div>
      <section className="panel">
        <div className="section-heading">
          <div>
            <h2>账号与凭据</h2>
            <p>账号独立保存，完整凭据组按会话管理</p>
          </div>
          <span className="count">{accounts.length} 个账号</span>
        </div>
        {accounts.length === 0 ? (
          <div className="empty">
            <div className="empty-icon">
              <Icon name="lock" size={25} />
            </div>
            <h3>从你的第一个账号开始</h3>
            <p>
              导入 API Key 或已有客户端的 JSON 凭据。
              <br />
              每个账号都保留自己的身份和凭据组。
            </p>
            <button className="secondary" onClick={onImport}>
              导入账号
              <Icon name="arrow" size={16} />
            </button>
          </div>
        ) : (
          <div className="account-list">
            {accounts.map((account) => {
              const credentials = snapshot.credentials.filter(
                (credential) => credential.accountId === account.id,
              );
              return (
                <article key={account.id} className="account-row">
                  <div className={`account-avatar ${provider}`}>
                    {account.label.slice(0, 1)}
                  </div>
                  <div className="grow">
                    <h3>{account.label}</h3>
                    <p>
                      {credentials
                        .map(
                          (credential) =>
                            `${credential.kind === "oauth" ? "OAuth" : "API Key"} · v${credential.version}`,
                        )
                        .join(" / ")}
                      <span className="separator">/</span>额度在悬浮窗更新
                    </p>
                    <code className="record-id">{account.id}</code>
                  </div>
                  <span className="tag amber">身份未验证</span>
                </article>
              );
            })}
          </div>
        )}
      </section>
      <section className="panel">
        <div className="section-heading">
          <div>
            <h2>模型连接</h2>
            <p>为声明的模型指定渠道与完整凭据组</p>
          </div>
          <button
            className="secondary"
            disabled={!accounts.length}
            onClick={onConnection}
          >
            <Icon name="plus" size={16} />
            添加连接
          </button>
        </div>
        {connections.length === 0 ? (
          <div className="inline-empty">
            <Icon name="link" />
            <span>尚未配置模型连接</span>
          </div>
        ) : (
          <div className="connection-list">
            {connections.map((connection) => (
              <article key={connection.id}>
                <div>
                  <h3>{connection.label}</h3>
                  <p>{connection.model}</p>
                  <code>{connection.baseUrl}</code>
                </div>
                <span className="tag">待验证</span>
              </article>
            ))}
          </div>
        )}
      </section>
      <p className="footnote">
        额度、套餐与用量在悬浮窗中实时更新。客户端写入功能尚未接通。
      </p>
    </>
  );
}
