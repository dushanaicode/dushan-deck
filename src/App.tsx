import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Icon } from "./components/Icon";
import {
  VaultForm,
  ImportForm,
  ConnectionForm,
} from "./features/accounts/AccountForms";
import { ProviderPage } from "./features/catalog/ProviderPage";
import { SettingsPage } from "./features/settings/SettingsPage";
import { Workspace } from "./features/workspace/Workspace";
import type { Provider } from "./lib/contracts";
import { api, desktop, errorMessage, useDeck } from "./lib/deck";

type Page = "workspace" | "settings" | Provider;
type Dialog = "vault" | "import" | "connection" | null;
const taskLabels = {
  queued: "等待中",
  running: "执行中",
  waiting_user: "等待用户",
  paused: "已暂停",
  succeeded: "已完成",
  failed: "失败",
  cancelled: "已取消",
  interrupted: "已中断",
};

export default function App() {
  const { snapshot, error, exiting, reload, setError } = useDeck();
  const [page, setPage] = useState<Page>("workspace");
  const [dialog, setDialog] = useState<Dialog>(null);
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const floating =
    new URLSearchParams(window.location.search).get("surface") === "float";
  async function action(work: () => Promise<unknown>) {
    setBusy(true);
    setError(null);
    try {
      await work();
      await reload();
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }
  function done(message: string) {
    setDialog(null);
    setNotice(message);
    void reload();
  }
  function openImport() {
    setDialog(snapshot!.vaultUnlocked ? "import" : "vault");
  }
  if (floating)
    return (
      <main className="float-shell">
        <header>
          <div className="brand compact">
            <span className="brand-icon">D</span>
            <strong>Dushan Deck</strong>
          </div>
          <button
            className="icon-button"
            aria-label="隐藏悬浮窗"
            onClick={() => void action(() => getCurrentWindow().close())}
          >
            <Icon name="close" size={17} />
          </button>
        </header>
        <span className="eyebrow">YOUR DECK, AT A GLANCE</span>
        <h1>随时，就位。</h1>
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
        {snapshot && (
          <>
            <div className="float-metric">
              <span>本地账号</span>
              <strong>{snapshot.accounts.length}</strong>
            </div>
            {snapshot.providers
              .filter((provider) =>
                snapshot.settings.favoriteProviders.includes(provider.id),
              )
              .map((provider) => (
                <div key={provider.id} className="float-row">
                  <span className={`provider-dot ${provider.id}`} />
                  <strong>{provider.name}</strong>
                  <span>
                    {
                      snapshot.accounts.filter(
                        (account) => account.provider === provider.id,
                      ).length
                    }{" "}
                    个账号
                  </span>
                </div>
              ))}
            <p className="muted">
              凭据库{snapshot.vaultUnlocked ? "已解锁" : "已锁定"} ·
              额度尚未接入
            </p>
          </>
        )}
        <button className="primary" onClick={() => void action(api.showMain)}>
          打开工作台
          <Icon name="arrow" size={17} />
        </button>
      </main>
    );
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-icon">D</span>
          <div>
            <strong>Dushan Deck</strong>
            <small>YOUR PERSONAL AI DESK</small>
          </div>
        </div>
        <div className="workspace-label">
          <span className="workspace-avatar">D</span>
          <div>
            个人工作空间<small>LOCAL WORKSPACE</small>
          </div>
          <span className="small-dot" />
        </div>
        <nav aria-label="主导航">
          <button
            className={page === "workspace" ? "nav-item selected" : "nav-item"}
            onClick={() => setPage("workspace")}
          >
            <Icon name="grid" />
            工作台
          </button>
          <p className="nav-label">模型专区</p>
          {snapshot?.providers
            .filter((provider) =>
              snapshot.settings.favoriteProviders.includes(provider.id),
            )
            .map((provider) => (
              <button
                key={provider.id}
                className={
                  page === provider.id ? "nav-item selected" : "nav-item"
                }
                onClick={() => setPage(provider.id)}
              >
                <span className={`mini-provider ${provider.id}`}>
                  {provider.id === "claude" ? "✳" : "◈"}
                </span>
                {provider.name}
                <span className="nav-count">
                  {
                    snapshot.accounts.filter(
                      (account) => account.provider === provider.id,
                    ).length
                  }
                </span>
              </button>
            ))}
          <p className="nav-label">探索与助手</p>
          <button className="nav-item" disabled>
            <Icon name="flask" />
            检测实验室<span className="soon">待开放</span>
          </button>
          <button className="nav-item" disabled>
            <Icon name="spark" />
            桌面伙伴<span className="soon">待开放</span>
          </button>
        </nav>
        <div className="sidebar-bottom">
          <button
            className={page === "settings" ? "nav-item selected" : "nav-item"}
            onClick={() => setPage("settings")}
          >
            <Icon name="settings" />
            设置
          </button>
          <div className="version">
            <span className="small-dot" />
            {desktop ? "本地桌面" : "浏览器预览"}
            <span>v0.1.0</span>
          </div>
        </div>
      </aside>
      <div className="main-shell">
        <header className="topbar">
          <div className="breadcrumb">
            工作空间<span>/</span>
            <strong>
              {page === "workspace"
                ? "工作台"
                : page === "settings"
                  ? "设置"
                  : page === "claude"
                    ? "Claude"
                    : "OpenAI"}
            </strong>
          </div>
          <div className="topbar-actions">
            <button
              className="text-button"
              disabled={!snapshot || busy}
              onClick={() => void action(api.showFloat)}
            >
              <Icon name="float" size={17} />
              <span>悬浮窗</span>
            </button>
            <span className="topbar-divider" />
            <button
              className="icon-button"
              disabled={!snapshot || busy}
              title="退出应用"
              aria-label="退出应用"
              onClick={() => void action(api.exit)}
            >
              <Icon name="power" size={18} />
            </button>
          </div>
        </header>
        <div className="content-grid">
          <main className="page-content">
            {error && (
              <div className="error-banner" role="alert">
                <strong>暂时无法完成操作</strong>
                <p>{error}</p>
                {desktop && (
                  <button className="text-button" onClick={() => void reload()}>
                    重新读取状态
                  </button>
                )}
              </div>
            )}
            {notice && (
              <div className="notice" role="status">
                <Icon name="check" size={17} />
                <span>{notice}</span>
                <button
                  aria-label="关闭提示"
                  className="icon-button"
                  onClick={() => setNotice("")}
                >
                  <Icon name="close" size={15} />
                </button>
              </div>
            )}
            {!snapshot && !error && (
              <p className="loading" role="status">
                正在连接本地工作台…
              </p>
            )}
            {snapshot &&
              (page === "workspace" ? (
                <Workspace
                  snapshot={snapshot}
                  onProvider={setPage}
                  onUnlock={() => setDialog("vault")}
                />
              ) : page === "settings" ? (
                <SettingsPage
                  snapshot={snapshot}
                  busy={busy}
                  onSave={(settings) =>
                    void action(() => api.saveSettings(settings))
                  }
                  onCheck={() => void action(api.checkStorage)}
                  onVault={() =>
                    snapshot.vaultUnlocked
                      ? void action(api.lock)
                      : setDialog("vault")
                  }
                />
              ) : (
                <ProviderPage
                  provider={page}
                  snapshot={snapshot}
                  onImport={openImport}
                  onConnection={() => setDialog("connection")}
                />
              ))}
            <footer className="page-footer">
              <span>DUSHAN DECK</span>
              <span>一个安静、有序的 AI 工作空间</span>
            </footer>
          </main>
          <aside className="activity-rail">
            <div className="rail-heading">
              <h2>工作台动态</h2>
              <Icon name="activity" size={18} />
            </div>
            <div className="rail-status">
              <span className="status-ring">
                <Icon name="layers" size={23} />
              </span>
              <h3>由本机掌握</h3>
              <p>
                账号、连接与任务记录
                <br />
                保留在你的设备上。
              </p>
            </div>
            <div className="rail-section">
              <span className="eyebrow">RECENT TASKS</span>
              <h3>最近任务</h3>
              {snapshot?.tasks.length ? (
                snapshot.tasks.map((task) => (
                  <article className="task-item" key={task.id}>
                    <span className={`task-dot ${task.state}`} />
                    <div>
                      <strong>本地数据库检查</strong>
                      <p>{task.message}</p>
                      <small>
                        {taskLabels[task.state]} ·{" "}
                        {new Date(task.createdAt * 1000).toLocaleTimeString(
                          "zh-CN",
                          { hour: "2-digit", minute: "2-digit" },
                        )}
                      </small>
                    </div>
                  </article>
                ))
              ) : (
                <div className="no-tasks">
                  <span>—</span>
                  <p>还没有任务记录</p>
                  <small>完成的操作会在这里留下足迹。</small>
                </div>
              )}
            </div>
            <div className="rail-note">
              <span className="eyebrow">MADE FOR YOUR FLOW</span>
              <p>
                先连接，
                <br />
                再探索更多可能。
              </p>
              <Icon name="spark" size={21} />
            </div>
          </aside>
        </div>
      </div>
      {snapshot && dialog === "vault" && (
        <VaultForm
          configured={snapshot.vaultConfigured}
          onClose={() => setDialog(null)}
          onDone={() => done("凭据库已解锁")}
        />
      )}
      {snapshot &&
        (page === "claude" || page === "openai") &&
        dialog === "import" && (
          <ImportForm
            provider={page}
            onClose={() => setDialog(null)}
            onDone={done}
          />
        )}
      {snapshot &&
        (page === "claude" || page === "openai") &&
        dialog === "connection" && (
          <ConnectionForm
            provider={page}
            snapshot={snapshot}
            onClose={() => setDialog(null)}
            onDone={() => done("模型连接已保存，授权与调用能力待验证")}
          />
        )}
      {exiting && (
        <div className="exit-overlay" role="status">
          <Icon name="power" />
          <h2>正在结束工作台</h2>
          <p>等待本地写入完成并释放后台资源…</p>
        </div>
      )}
    </div>
  );
}
