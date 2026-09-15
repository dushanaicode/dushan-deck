import type { Settings, Snapshot } from "../../lib/contracts";
import { Icon } from "../../components/Icon";
import { UsageSources } from "./UsageSources";

export function SettingsPage({
  snapshot,
  busy,
  onSave,
  onCheck,
  onVault,
}: {
  snapshot: Snapshot;
  busy: boolean;
  onSave: (settings: Settings) => void;
  onCheck: () => void;
  onVault: () => void;
}) {
  return (
    <>
      <div className="page-heading">
        <div>
          <span className="eyebrow">PREFERENCES</span>
          <h1>设置</h1>
          <p>让工作台按你的习惯运行。</p>
        </div>
      </div>
      <section className="panel">
        <div className="section-heading">
          <div>
            <h2>桌面与专区</h2>
            <p>设置由本地数据库保存，重启后恢复</p>
          </div>
          <Icon name="settings" />
        </div>
        <label className="setting-row">
          <span>
            <strong>启动时显示悬浮窗</strong>
            <small>关闭悬浮窗只隐藏自己；主窗口关闭后收起到托盘</small>
          </span>
          <input
            type="checkbox"
            checked={snapshot.settings.floatEnabled}
            disabled={busy}
            onChange={(event) =>
              onSave({
                ...snapshot.settings,
                floatEnabled: event.target.checked,
              })
            }
          />
        </label>
        {snapshot.providers.map((provider) => (
          <label className="setting-row" key={provider.id}>
            <span>
              <strong>收藏 {provider.name} 专区</strong>
              <small>显示在工作台与侧栏</small>
            </span>
            <input
              type="checkbox"
              disabled={busy}
              checked={snapshot.settings.favoriteProviders.includes(
                provider.id,
              )}
              onChange={(event) =>
                onSave({
                  ...snapshot.settings,
                  favoriteProviders: event.target.checked
                    ? [...snapshot.settings.favoriteProviders, provider.id]
                    : snapshot.settings.favoriteProviders.filter(
                        (id) => id !== provider.id,
                      ),
                })
              }
            />
          </label>
        ))}
      </section>
      <section className="panel">
        <div className="section-heading">
          <div>
            <h2>本地存储</h2>
            <p>数据库版本 {snapshot.schemaVersion} · 无云端同步</p>
          </div>
          <Icon name="lock" />
        </div>
        <div className="setting-row">
          <span>
            <strong>
              凭据库
              {snapshot.vaultUnlocked
                ? "已解锁"
                : snapshot.vaultConfigured
                  ? "已锁定"
                  : "尚未创建"}
            </strong>
            <small>完整凭据组加密保存，口令不落盘</small>
          </span>
          <button className="secondary" disabled={busy} onClick={onVault}>
            {snapshot.vaultUnlocked
              ? "锁定凭据库"
              : snapshot.vaultConfigured
                ? "解锁"
                : "创建凭据库"}
          </button>
        </div>
        <div className="setting-row">
          <span>
            <strong>检查数据库</strong>
            <small>核验 SQLite 完整性和记录引用，结果保留在任务历史</small>
          </span>
          <button className="secondary" disabled={busy} onClick={onCheck}>
            运行检查
          </button>
        </div>
      </section>
      <UsageSources snapshot={snapshot} />
      <div className="keyboard-hint">
        <kbd>Ctrl / ⌘</kbd> + <kbd>Shift</kbd> + <kbd>D</kbd>
        <span>打开主窗口</span>
      </div>
    </>
  );
}
