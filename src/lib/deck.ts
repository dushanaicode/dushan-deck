import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ConnectionRequest,
  ImportRequest,
  ImportResult,
  Settings,
  Snapshot,
} from "./contracts";

export const desktop = isTauri();
export const api = {
  snapshot: () => invoke<Snapshot>("get_snapshot"),
  unlock: (password: string) => invoke<void>("unlock_vault", { password }),
  lock: () => invoke<void>("lock_vault"),
  import: (request: ImportRequest) =>
    invoke<ImportResult>("import_account", { request }),
  createConnection: (request: ConnectionRequest) =>
    invoke<string>("create_connection", { request }),
  saveSettings: (settings: Settings) =>
    invoke<void>("save_settings", { settings }),
  checkStorage: () => invoke<string>("check_storage"),
  showMain: () => invoke<void>("show_main"),
  showFloat: () => invoke<void>("show_float"),
  exit: () => invoke<void>("request_exit"),
};

export function errorMessage(error: unknown): string {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  )
    return error.message;
  return typeof error === "string" ? error : "操作失败，请检查应用状态后重试";
}

export function useDeck() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [exiting, setExiting] = useState(false);
  const sequence = useRef(0);
  const mounted = useRef(false);
  const reload = useCallback(async () => {
    const request = ++sequence.current;
    try {
      const data = await api.snapshot();
      if (mounted.current && request === sequence.current) {
        setSnapshot(data);
        setError(null);
      }
    } catch (error) {
      if (mounted.current && request === sequence.current)
        setError(errorMessage(error));
    }
  }, []);
  useEffect(() => {
    mounted.current = true;
    if (!desktop) {
      setError(
        "此页面需要 Dushan Deck 桌面应用连接本地数据。请通过项目启动命令打开桌面窗口。",
      );
      return () => {
        mounted.current = false;
      };
    }
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    async function connect() {
      for (const [name, callback] of [
        ["deck:changed", () => void reload()],
        ["deck:exiting", () => setExiting(true)],
        [
          "deck:error",
          (event: { payload: unknown }) =>
            setError(errorMessage(event.payload)),
        ],
      ] as const) {
        const unlisten = await listen(name, callback);
        if (disposed) unlisten();
        else unlisteners.push(unlisten);
      }
      if (!disposed) await reload();
    }
    void connect().catch((error: unknown) => {
      if (!disposed) setError(errorMessage(error));
    });
    const onFocus = () => {
      if (document.visibilityState === "visible") void reload();
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onFocus);
    return () => {
      disposed = true;
      mounted.current = false;
      sequence.current++;
      unlisteners.forEach((unlisten) => unlisten());
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onFocus);
    };
  }, [reload]);
  return { snapshot, error, exiting, reload, setError };
}
