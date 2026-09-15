import { createRoot } from "react-dom/client";
import { useCallback, useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { FloatingPanel } from "./FloatingPanel";
import type { FloatPreferences, FloatState, QuotaSnapshot } from "./types";
import { errorMessage } from "../../lib/deck";
import defaultBackground from "./bg-default.jpg";
import "./quota.css";

function FloatingWindow() {
  const [state, setState] = useState<FloatState | null>(null);
  const [snapshot, setSnapshot] = useState<QuotaSnapshot>({
    results: [],
    state: "cached",
    fetchedAt: null,
    error: "",
  });
  const [refreshing, setRefreshing] = useState(false),
    [error, setError] = useState("");
  const [refreshPulse, setRefreshPulse] = useState(0);
  const file = useRef<HTMLInputElement>(null),
    saveQueue = useRef(Promise.resolve());
  const alive = useRef(true),
    refreshingRef = useRef(false);
  const platform = navigator.userAgent.includes("Mac")
    ? "macos"
    : navigator.userAgent.includes("Windows")
      ? "windows"
      : "linux";
  const readQuota = useCallback(async () => {
    const data = await invoke<QuotaSnapshot>("get_quota_snapshot");
    if (alive.current) setSnapshot(data);
  }, []);
  const refresh = useCallback(async () => {
    if (refreshingRef.current) return;
    refreshingRef.current = true;
    setRefreshing(true);
    setError("");
    try {
      const data = await invoke<QuotaSnapshot>("refresh_quotas", {
        force: true,
      });
      if (alive.current) {
        setSnapshot(data);
        if (data.state === "cached" || data.state === "fresh")
          setRefreshPulse((value) => value + 1);
      }
    } catch (error) {
      if (alive.current) setError(errorMessage(error));
    } finally {
      refreshingRef.current = false;
      if (alive.current) setRefreshing(false);
    }
  }, []);
  useEffect(() => {
    alive.current = true;
    if (!isTauri()) {
      setError("请从 Dushan Deck 桌面应用打开悬浮窗");
      return () => {
        alive.current = false;
      };
    }
    const stops: (() => void)[] = [];
    let disposed = false;
    async function start() {
      for (const name of ["deck:quota", "deck:changed"]) {
        const stop = await listen(name, () => {
          void readQuota().catch((error: unknown) =>
            setError(errorMessage(error)),
          );
        });
        if (disposed) stop();
        else stops.push(stop);
      }
      const initial = await invoke<FloatState>("get_float_state");
      if (!disposed) {
        setState(initial);
        await readQuota();
      }
    }
    void start().catch((error: unknown) => {
      if (!disposed) setError(errorMessage(error));
    });
    const focus = () => {
      void readQuota().catch((error: unknown) => setError(errorMessage(error)));
    };
    window.addEventListener("focus", focus);
    return () => {
      disposed = true;
      alive.current = false;
      stops.forEach((stop) => stop());
      window.removeEventListener("focus", focus);
    };
  }, [readQuota]);
  function change(preferences: FloatPreferences, persist = true) {
    setState((current) => ({ ...current!, preferences }));
    if (preferences.alpha !== state!.preferences.alpha)
      void invoke("preview_float_alpha", { alpha: preferences.alpha }).catch(
        (error: unknown) => setError(errorMessage(error)),
      );
    if (persist)
      saveQueue.current = saveQueue.current
        .then(async () => {
          await invoke("save_float_preferences", { preferences });
          if (alive.current) setError("");
        })
        .catch((error: unknown) => {
          if (alive.current) setError(errorMessage(error));
        });
  }
  async function action(
    name:
      | "main"
      | "hide"
      | "drag"
      | "resize"
      | "pickBackground"
      | "clearBackground",
  ) {
    try {
      switch (name) {
        case "main":
          await invoke("show_main");
          break;
        case "hide":
          await getCurrentWindow().close();
          break;
        case "drag":
          await getCurrentWindow().startDragging();
          break;
        case "resize":
          await getCurrentWindow().startResizeDragging("SouthEast");
          break;
        case "pickBackground":
          file.current!.click();
          break;
        case "clearBackground":
          await invoke("save_float_background", { background: null });
          setState((current) => ({ ...current!, background: null }));
          break;
      }
    } catch (error) {
      setError(errorMessage(error));
    }
  }
  async function selected(file: File) {
    try {
      if (file.size > 30 * 1024 * 1024) throw new Error("图片超过 30MB");
      const bitmap = await createImageBitmap(file);
      bitmap.close();
      const background = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(reader.result as string);
        reader.onerror = () => reject(new Error("无法读取图片"));
        reader.readAsDataURL(file);
      });
      await invoke("save_float_background", { background });
      setState((current) => ({ ...current!, background }));
      setError("");
    } catch (error) {
      setError(errorMessage(error));
    }
  }
  if (!state)
    return (
      <div className="txt" style={{ padding: 12 }} role="status">
        {error || "加载中…"}
      </div>
    );
  return (
    <>
      <FloatingPanel
        settings={state.preferences}
        background={state.background ?? defaultBackground}
        snapshot={snapshot}
        refreshing={refreshing}
        refreshPulse={refreshPulse}
        error={error}
        platform={platform}
        change={change}
        refresh={() => void refresh()}
        action={(name) => void action(name)}
      />
      <input
        type="file"
        hidden
        ref={file}
        accept="image/png,image/jpeg,image/webp,image/gif,image/bmp"
        aria-label="悬浮窗背景图片"
        onChange={(event) => {
          const selectedFile = event.target.files?.[0];
          if (selectedFile) void selected(selectedFile);
          event.target.value = "";
        }}
      />
    </>
  );
}
createRoot(document.getElementById("root")!).render(<FloatingWindow />);
