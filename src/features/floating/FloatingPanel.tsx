import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent, ReactNode } from "react";
import type {
  FloatPreferences,
  FloatTheme,
  QuotaAccount,
  QuotaSnapshot,
} from "./types";
import { accountKey, QuotaRow, UsageBlock } from "./presentation";

const themes: [FloatTheme, string][] = [
  ["dark", "暗夜"],
  ["ocean", "湛蓝"],
  ["forest", "翠绿"],
  ["violet", "紫檀"],
  ["paper", "浅白"],
];
type WindowAction =
  "main" | "hide" | "drag" | "resize" | "pickBackground" | "clearBackground";
interface Props {
  settings: FloatPreferences;
  background: string;
  snapshot: QuotaSnapshot;
  refreshing: boolean;
  refreshPulse: number;
  error: string;
  change: (settings: FloatPreferences, persist?: boolean) => void;
  refresh: () => void;
  action: (action: WindowAction) => void;
  platform: string;
}
function keyboardClick(event: KeyboardEvent<HTMLElement>, action: () => void) {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    action();
  }
}
function Control({
  id,
  title,
  className = "",
  action,
  children,
}: {
  id: string;
  title: string;
  className?: string;
  action: () => void;
  children: ReactNode;
}) {
  return (
    <span
      id={id}
      className={className}
      title={title}
      aria-label={title}
      role="button"
      tabIndex={0}
      onClick={action}
      onKeyDown={(event) => keyboardClick(event, action)}
    >
      {children}
    </span>
  );
}
export function FloatingPanel({
  settings: s,
  background,
  snapshot,
  refreshing,
  refreshPulse,
  error,
  change,
  refresh,
  action,
  platform,
}: Props) {
  const [settingsOpen, setSettingsOpen] = useState(false),
    [now, setNow] = useState(Date.now());
  const [dragging, setDragging] = useState<string | null>(null);
  const [dragOrder, setDragOrder] = useState<string[] | null>(null);
  const drag = useRef<{
    key: string;
    y: number;
    moved: boolean;
    element: HTMLElement;
    pointerId: number;
  } | null>(null);
  const list = useRef<HTMLDivElement>(null),
    cfg = useRef<HTMLDivElement>(null);
  const latest = useRef(s);
  latest.current = s;
  const oldBars = useRef(new Map<string, string>());
  const structure = useRef("");
  const previousBackground = useRef(background);
  useEffect(() => {
    if (refreshPulse > 0 && latest.current.animations) {
      const panel = document.getElementById("panel")!;
      panel.classList.remove("refreshed");
      void panel.offsetWidth;
      panel.classList.add("refreshed");
    }
  }, [refreshPulse]);
  useEffect(() => {
    if (previousBackground.current !== background && s.animations) {
      document
        .getElementById("bg")!
        .animate([{ opacity: 0.4 }, { opacity: 1 }], {
          duration: 180,
          easing: "cubic-bezier(.2,.7,.2,1)",
        });
    }
    previousBackground.current = background;
  }, [background, s.animations]);
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  useEffect(() => {
    document.body.dataset.theme = s.theme;
    document.body.classList.toggle("motion", s.animations);
    document.body.classList.toggle("bg", !!background);
    document.documentElement.classList.toggle("mac", platform === "macos");
    if (!s.animations)
      document.getAnimations().forEach((animation) => animation.cancel());
  }, [s.theme, s.animations, background, platform]);
  const order = dragOrder ?? s.cardOrder;
  const rank = (key: string) => {
    const index = order.indexOf(key);
    return index < 0 ? order.length : index;
  };
  const accounts = snapshot.results
    .filter((item) => s.show[item.title] !== false)
    .sort((a, b) => rank(accountKey(a)) - rank(accountKey(b)));
  useEffect(() => {
    const element = list.current!;
    const keys = accounts.map(accountKey).join("|");
    if (s.animations && keys !== structure.current)
      element.animate([{ opacity: 0.4 }, { opacity: 1 }], {
        duration: 180,
        easing: "cubic-bezier(.2,.7,.2,1)",
      });
    structure.current = keys;
    element.querySelectorAll<HTMLElement>("[data-bar]").forEach((bar) => {
      const previous = oldBars.current.get(bar.dataset.bar!) ?? "0%";
      if (s.animations && previous !== bar.style.width)
        bar.animate([{ width: previous }, { width: bar.style.width }], {
          duration: 420,
          easing: "cubic-bezier(.2,.7,.2,1)",
        });
      oldBars.current.set(bar.dataset.bar!, bar.style.width);
    });
  }, [snapshot, s.animations, accounts]);
  const toggleSettings = () => {
    setSettingsOpen((open) => !open);
    if (s.animations)
      (settingsOpen ? list.current! : cfg.current!).animate(
        [{ opacity: 0.4 }, { opacity: 1 }],
        { duration: 180, easing: "cubic-bezier(.2,.7,.2,1)" },
      );
  };
  function startCard(event: PointerEvent<HTMLElement>, key: string) {
    if (event.button !== 0 || accounts.length < 2) return;
    const card = event.currentTarget.closest<HTMLElement>(".card")!;
    card.setPointerCapture(event.pointerId);
    drag.current = {
      key,
      y: event.clientY,
      moved: false,
      element: card,
      pointerId: event.pointerId,
    };
    event.preventDefault();
  }
  function moveCard(event: PointerEvent) {
    const current = drag.current;
    if (!current || (!current.moved && Math.abs(event.clientY - current.y) < 4))
      return;
    current.moved = true;
    setDragging(current.key);
    const cards = [...list.current!.querySelectorAll<HTMLElement>(".card")];
    const before = cards.find(
      (card) =>
        card.dataset.key !== current.key &&
        event.clientY <
          card.getBoundingClientRect().top +
            card.getBoundingClientRect().height / 2,
    );
    const keys = cards
      .map((card) => card.dataset.key!)
      .filter((key) => key !== current.key);
    const index = before ? keys.indexOf(before.dataset.key!) : keys.length;
    keys.splice(index, 0, current.key);
    setDragOrder(keys);
  }
  function endCard() {
    const current = drag.current;
    if (!current) return;
    if (current.element.hasPointerCapture(current.pointerId))
      current.element.releasePointerCapture(current.pointerId);
    drag.current = null;
    setDragging(null);
    if (current.moved) {
      const visible = [
        ...list.current!.querySelectorAll<HTMLElement>(".card"),
      ].map((card) => card.dataset.key!);
      change({
        ...s,
        cardOrder: [
          ...visible,
          ...s.cardOrder.filter((key) => !visible.includes(key)),
        ],
      });
    }
    setDragOrder(null);
  }
  function toggleShow(key: string) {
    change({
      ...s,
      show: {
        ...s.show,
        [key]: key === "#usage" ? !s.show[key] : s.show[key] === false,
      },
    });
  }
  const stamp = snapshot.fetchedAt
    ? new Date(snapshot.fetchedAt).toLocaleTimeString([], {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      })
    : "";
  const failed =
    !!error || snapshot.state === "error" || snapshot.state === "stale";
  const status =
    error ||
    (refreshing
      ? "刷新中…"
      : snapshot.state === "locked"
        ? "凭据库已锁定"
        : snapshot.state === "error"
          ? "刷新失败"
          : snapshot.state === "stale"
            ? `旧数据 · ${stamp}`
            : stamp
              ? `已更新 · ${stamp}`
              : "准备中");
  const statusTitle =
    error ||
    snapshot.error ||
    (snapshot.state === "stale" ? "刷新失败，保留上次数据" : status);
  const changeRange = (
    key: "alpha" | "bgDim" | "bgBlur" | "glassBlur",
    value: number,
  ) => change({ ...latest.current, [key]: value }, false);
  const range = (
    id: string,
    key: "alpha" | "bgDim" | "bgBlur" | "glassBlur",
    min: number,
    max: number,
    label: string,
    suffix: string,
  ) => (
    <>
      <div className="albl">
        <span>{label}</span>
        <span id={id.replace("R", "Val")}>{`${s[key]}${suffix}`}</span>
      </div>
      <input
        type="range"
        id={id}
        aria-label={label}
        min={min}
        max={max}
        value={s[key]}
        onChange={(event) => changeRange(key, +event.target.value)}
        onPointerUp={() => change(latest.current)}
        onKeyUp={() => change(latest.current)}
        onBlur={() => change(latest.current)}
      />
    </>
  );
  const accountCard = (item: QuotaAccount) => {
    const key = accountKey(item),
      expiration = item.subEnd ? new Date(item.subEnd) : null;
    const date = expiration
      ? `${expiration.getFullYear()}-${String(expiration.getMonth() + 1).padStart(2, "0")}-${String(expiration.getDate()).padStart(2, "0")}`
      : "";
    const retry = new Date(item.retryAt * 1000);
    return (
      <div
        className={`card${dragging === key ? " dragging" : ""}`}
        data-key={key}
        key={key}
        onPointerMove={moveCard}
        onPointerUp={endCard}
        onPointerCancel={endCard}
      >
        <div
          className="name"
          title="按住上下拖动可调整顺序"
          onPointerDown={(event) => startCard(event, key)}
        >
          <span className="grip" aria-hidden="true" />
          <b>{item.title}</b>
          {s.show["#plan"] !== false && item.plan && (
            <span
              className="pill"
              title={
                item.planDetail
                  ? `${item.plan} · 接口原值 ${item.planDetail}`
                  : item.plan
              }
            >
              {item.plan}
            </span>
          )}
          {s.show["#email"] !== false && item.email && (
            <span>{item.email}</span>
          )}
        </div>
        {s.show["#plan"] !== false && date && (
          <div className="sub">
            {item.subStatus === "expired" ? "已到期" : "到期"} {date}
          </div>
        )}
        {item.ok &&
          item.windows.map((row) => (
            <QuotaRow
              key={row.name}
              row={row}
              account={key}
              settings={s}
              now={now}
              change={change}
            />
          ))}
        {item.pending && <div className="txt">{item.pending}</div>}
        {item.notice && (
          <div className="note">
            {`${item.notice}，${item.ok ? "显示上次数据，" : ""}${String(retry.getHours()).padStart(2, "0")}:${String(retry.getMinutes()).padStart(2, "0")} 自动重试`}
          </div>
        )}
        {(item.error || (!item.ok && !item.notice && !item.pending)) && (
          <div className="err">{item.error || "失败"}</div>
        )}
        <UsageBlock account={item} settings={s} change={change} />
      </div>
    );
  };
  return (
    <>
      <div
        id="bg"
        style={{
          backgroundImage: `url(${JSON.stringify(background)})`,
          filter: s.bgBlur ? `blur(${s.bgBlur}px)` : "",
          transform: s.bgBlur ? `scale(${1 + (2 * s.bgBlur) / 250})` : "",
        }}
      />
      <div
        id="scrim"
        style={{
          background: `rgba(16,19,26,${s.bgDim / 100})`,
          backdropFilter: `blur(${s.glassBlur}px)`,
          WebkitBackdropFilter: `blur(${s.glassBlur}px)`,
        }}
      />
      <div
        id="panel"
        className={settingsOpen ? "settings-open" : ""}
        style={
          platform === "macos"
            ? { borderRadius: s.rounded ? 14 : 0 }
            : undefined
        }
      >
        <div
          className="hdr"
          id="hdr"
          onPointerDown={(event) => {
            if (
              event.button === 0 &&
              !(event.target as HTMLElement).closest(".ico")
            ) {
              event.preventDefault();
              action("drag");
            }
          }}
        >
          <span
            className={`dot${refreshing ? " loading" : failed ? " error" : ""}`}
            id="stateDot"
          />
          <b>Quota</b>
          <span
            className="status"
            id="status"
            title={statusTitle}
            role="status"
          >
            {status}
          </span>
          <span className="sp" />
          <Control
            id="tWeb"
            className="ico"
            title="打开主窗口"
            action={() => action("main")}
          >
            🌐
          </Control>
          <Control
            id="tPin"
            className={`ico pin${s.onTop ? " on" : ""}`}
            title="置顶"
            action={() => change({ ...s, onTop: !s.onTop })}
          >
            <i>📌</i>
          </Control>
          <span
            className={`ico${settingsOpen ? " on" : ""}`}
            id="tCfg"
            title="设置"
            role="button"
            tabIndex={0}
            aria-controls="cfg"
            aria-expanded={settingsOpen}
            onClick={toggleSettings}
            onKeyDown={(event) => keyboardClick(event, toggleSettings)}
          >
            ⚙
          </span>
          <Control
            id="tRef"
            className={`ico${refreshing ? " loading" : ""}`}
            title="刷新"
            action={refresh}
          >
            ↻
          </Control>
          <Control
            id="tQuit"
            className="ico"
            title="隐藏悬浮窗"
            action={() => action("hide")}
          >
            ✕
          </Control>
        </div>
        <div id="body">
          <div id="list" ref={list}>
            {accounts.length ? (
              accounts.map(accountCard)
            ) : (
              <div className="txt" style={{ padding: "6px 0" }}>
                {snapshot.state === "error"
                  ? "刷新失败，请稍后重试"
                  : "没有账号"}
              </div>
            )}
          </div>
        </div>
        <div id="cfg" ref={cfg}>
          <div className="sec">窗口</div>
          {range("alphaR", "alpha", 20, 100, "不透明度", "%")}
          <div id="winOpts">
            <Control
              id="roundedChip"
              className={`chip${s.rounded ? " on" : ""}`}
              title="圆角窗口"
              action={() => change({ ...s, rounded: !s.rounded })}
            >
              圆角窗口
            </Control>
            <button
              type="button"
              className={`chip${s.animations ? " on" : ""}`}
              id="animationChip"
              aria-pressed={s.animations}
              onClick={() => change({ ...s, animations: !s.animations })}
            >
              动画效果
            </button>
          </div>
          <div className="sec">背景图片</div>
          <div className="bgrow">
            <Control
              id="bgPick"
              className="bgbtn"
              title="选择图片"
              action={() => action("pickBackground")}
            >
              选择图片…
            </Control>
            <Control
              id="bgClear"
              className="bgbtn"
              title="恢复默认背景"
              action={() => action("clearBackground")}
            >
              恢复默认
            </Control>
          </div>
          <div id="bgDimWrap">
            {range("bgDimR", "bgDim", 0, 90, "背景压暗", "%")}
            {range("bgBlurR", "bgBlur", 0, 30, "背景模糊", "px")}
            {range("glassBlurR", "glassBlur", 0, 30, "毛玻璃模糊", "px")}
          </div>
          <div className="sec">配色主题</div>
          <div id="themes">
            {themes.map(([theme, label]) => (
              <span
                className={`chip${s.theme === theme ? " on" : ""}`}
                data-t={theme}
                key={theme}
                role="button"
                tabIndex={0}
                onClick={() => change({ ...s, theme })}
                onKeyDown={(event) =>
                  keyboardClick(event, () => change({ ...s, theme }))
                }
              >
                {label}
              </span>
            ))}
          </div>
          <div className="sec">显示内容</div>
          <div id="pick">
            {[...new Set(snapshot.results.map((item) => item.title))].map(
              (title) => (
                <span
                  className={`chip${s.show[title] !== false ? " on" : ""}`}
                  data-n={title}
                  key={title}
                  role="button"
                  tabIndex={0}
                  onClick={() => toggleShow(title)}
                  onKeyDown={(event) =>
                    keyboardClick(event, () => toggleShow(title))
                  }
                >
                  {title}
                </span>
              ),
            )}
          </div>
          <div id="orderRow">
            <button
              type="button"
              className="chip"
              id="orderReset"
              title="清除手动排序，恢复接口返回的顺序"
              onClick={() => change({ ...s, cardOrder: [] })}
            >
              恢复默认顺序
            </button>
          </div>
          <div className="sec">卡片信息</div>
          <div id="meta">
            {[
              ["#email", "邮箱"],
              ["#plan", "套餐"],
              ["#usage", "用量信息"],
            ].map(([key, label]) => (
              <span
                className={`chip${key === "#usage" ? (s.show[key] ? " on" : "") : s.show[key] !== false ? " on" : ""}`}
                data-n={key}
                key={key}
                role="button"
                tabIndex={0}
                onClick={() => toggleShow(key)}
                onKeyDown={(event) =>
                  keyboardClick(event, () => toggleShow(key))
                }
              >
                {label}
              </span>
            ))}
          </div>
        </div>
        <div
          className="rs"
          id="rsHdl"
          title="拖动调整大小"
          onPointerDown={(event) => {
            if (event.button === 0) {
              event.preventDefault();
              event.stopPropagation();
              action("resize");
            }
          }}
        />
      </div>
    </>
  );
}
