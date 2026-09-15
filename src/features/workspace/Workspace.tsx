import type { Provider, Snapshot } from "../../lib/contracts";
import { Icon } from "../../components/Icon";

export function Workspace({
  snapshot,
  onProvider,
  onUnlock,
}: {
  snapshot: Snapshot;
  onProvider: (provider: Provider) => void;
  onUnlock: () => void;
}) {
  const providers = snapshot.providers.filter((provider) =>
    snapshot.settings.favoriteProviders.includes(provider.id),
  );
  return (
    <>
      <div className="page-heading">
        <div>
          <span className="eyebrow">YOUR LOCAL AI WORKSPACE</span>
          <h1>
            工作台<span className="heading-period">.</span>
          </h1>
          <p>让模型、账号和工作流，各就其位。</p>
        </div>
        <span className="local-badge">
          <i />
          本地优先
        </span>
      </div>
      <section className="welcome">
        <div className="welcome-content">
          <span className="eyebrow">A SPACE FOR YOUR MODELS</span>
          <h2>
            把你的 AI，
            <br />
            放在同一张桌面。
          </h2>
          <p>
            从模型专区开始。管理独立账号，连接日常工具，
            <br className="wide-only" />
            在你的设备上保留每一步的状态。
          </p>
          <button className="primary" onClick={() => onProvider("claude")}>
            管理模型与账号
            <Icon name="arrow" size={17} />
          </button>
        </div>
        <div className="deck-art" aria-hidden="true">
          <div className="art-orbit orbit-one" />
          <div className="art-orbit orbit-two" />
          <div className="art-card card-back">A</div>
          <div className="art-card card-middle">✳</div>
          <div className="art-card card-front">
            <span>DU</span>
            <span>
              SHAN<span className="art-period">.</span>
            </span>
            <small>YOUR MODELS. YOUR DECK.</small>
          </div>
          <span className="art-cross cross-one">+</span>
          <span className="art-cross cross-two">+</span>
        </div>
      </section>
      <div className="stats-grid">
        <div>
          <span>已保存账号</span>
          <strong>
            {snapshot.accounts.length.toString().padStart(2, "0")}
          </strong>
          <small>独立身份与凭据组</small>
        </div>
        <div>
          <span>模型连接</span>
          <strong>
            {snapshot.connections.length.toString().padStart(2, "0")}
          </strong>
          <small>由你配置的服务渠道</small>
        </div>
        <div>
          <span>凭据库</span>
          <strong className="text-value">
            {snapshot.vaultUnlocked
              ? "已解锁"
              : snapshot.vaultConfigured
                ? "已锁定"
                : "待创建"}
          </strong>
          <small>
            {snapshot.vaultUnlocked ? "退出后自动锁定" : "本地加密保存"}
          </small>
        </div>
      </div>
      <div className="section-heading outside">
        <div>
          <h2>你的模型专区</h2>
          <p>同一处管理，不同账号各自独立</p>
        </div>
        <span className="section-number">01 — MODEL SPACES</span>
      </div>
      <div className="provider-grid">
        {providers.map((provider) => (
          <button
            className={`provider-card ${provider.id}`}
            key={provider.id}
            onClick={() => onProvider(provider.id)}
          >
            <div className="provider-card-top">
              <span className={`provider-mark ${provider.id}`}>
                {provider.id === "claude" ? "✳" : "◈"}
              </span>
              <Icon name="arrow" size={19} />
            </div>
            <h3>{provider.name}</h3>
            <p>{provider.description}</p>
            <div className="provider-card-bottom">
              <span>
                {
                  snapshot.accounts.filter(
                    (account) => account.provider === provider.id,
                  ).length
                }{" "}
                个账号
              </span>
              <span>进入专区</span>
            </div>
          </button>
        ))}
      </div>
      {providers.length === 0 && (
        <p className="inline-empty">暂无收藏专区，可在设置中重新启用。</p>
      )}
      {!snapshot.vaultUnlocked && (
        <div className="vault-callout">
          <Icon name="lock" />
          <div>
            <h3>
              {snapshot.vaultConfigured
                ? "解锁你的本地凭据库"
                : "为账号准备一个本地凭据库"}
            </h3>
            <p>凭据整组加密保存，窗口仅展示必要摘要。</p>
          </div>
          <button className="text-button" onClick={onUnlock}>
            {snapshot.vaultConfigured ? "解锁" : "开始设置"}
            <Icon name="arrow" size={17} />
          </button>
        </div>
      )}
    </>
  );
}
