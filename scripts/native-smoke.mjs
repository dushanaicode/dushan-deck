import { spawn } from "node:child_process";
import { mkdirSync, openSync, closeSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { createServer } from "node:net";
import { chromium, expect } from "@playwright/test";

const root = process.cwd();
const evidence = resolve(root, `Temp/verification/native-${Date.now()}`);
const stateRoot = resolve(evidence, "state");
mkdirSync(evidence, { recursive: true });
if (!process.env.DECK_TEST_ROOT)
  throw new Error(
    "Use the project environment before running native verification.",
  );
const portServer = createServer();
await new Promise((done) => portServer.listen(0, "127.0.0.1", done));
const port = portServer.address().port;
await new Promise((done) => portServer.close(done));
const executable = resolve(root, "Temp/build/rust/debug/dushan-deck.exe");
let child;
let browser;
let main;
const failures = [];
const checks = [];
const measurements = { launches: [], exits: [], idleProcesses: [] };
const logs = openSync(resolve(evidence, "desktop.log"), "a");

async function nativeWindow(action = "read") {
  const probe = spawn(
    "powershell.exe",
    [
      "-NoProfile",
      "-File",
      "scripts/native-window.ps1",
      "-DeckProcessId",
      String(child.pid),
      "-Action",
      action,
    ],
    { cwd: root, windowsHide: true, env: process.env },
  );
  let output = "",
    error = "";
  probe.stdout.on("data", (chunk) => {
    output += chunk;
  });
  probe.stderr.on("data", (chunk) => {
    error += chunk;
  });
  const code = await new Promise((done, reject) => {
    probe.on("error", reject);
    probe.on("exit", done);
  });
  if (code !== 0) throw new Error(error);
  return JSON.parse(output);
}

async function ownedProcesses() {
  const probe = spawn(
    "powershell.exe",
    ["-NoProfile", "-File", "scripts/native-processes.ps1"],
    {
      cwd: root,
      windowsHide: true,
      env: { ...process.env, DECK_VERIFY_STATE_ROOT: stateRoot },
    },
  );
  let output = "";
  probe.stdout.on("data", (chunk) => {
    output += chunk;
  });
  probe.stderr.on("data", (chunk) => {
    failures.push(chunk.toString());
  });
  const code = await new Promise((done, reject) => {
    probe.on("error", reject);
    probe.on("exit", done);
  });
  expect(code).toBe(0);
  return JSON.parse(output);
}

async function start() {
  const startedAt = performance.now();
  child = spawn(executable, ["--state-root", stateRoot, "--offline"], {
    cwd: root,
    windowsHide: true,
    stdio: ["ignore", logs, logs],
    env: {
      ...process.env,
      WEBVIEW2_USER_DATA_FOLDER: resolve(stateRoot, "webview"),
      DECK_DEVTOOLS_PORT: String(port),
    },
  });
  child.on("error", (error) => failures.push(error.message));
  await expect
    .poll(
      async () => {
        if (child.exitCode !== null)
          throw new Error(`Desktop exited early: ${child.exitCode}`);
        try {
          return (await fetch(`http://127.0.0.1:${port}/json/version`)).ok;
        } catch {
          return false;
        }
      },
      { timeout: 20000 },
    )
    .toBe(true);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  await expect.poll(() => browser.contexts()[0].pages().length).toBe(2);
  main = browser
    .contexts()[0]
    .pages()
    .find((page) => !page.url().includes("float.html"));
  main.on("pageerror", (error) => failures.push(error.message));
  await expect(
    main.getByRole("heading", { name: /^工作台\s*\.$/, level: 1 }),
  ).toBeVisible();
  measurements.launches.push({
    untilUiAssertionMs: Math.round(performance.now() - startedAt),
    includesCdpPolling: true,
  });
}

async function invoke(page, command, args = {}) {
  return page.evaluate(
    ({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args),
    { command, args },
  );
}

async function exit() {
  const startedAt = performance.now();
  const ended = new Promise((done) => child.once("exit", done));
  await main.getByRole("button", { name: "退出应用", exact: true }).click();
  let timer;
  const deadline = new Promise((_, reject) => {
    timer = setTimeout(
      () => reject(new Error("Desktop exit timed out")),
      12000,
    );
  });
  let code;
  try {
    code = await Promise.race([ended, deadline]);
  } finally {
    clearTimeout(timer);
  }
  expect(code).toBe(0);
  measurements.exits.push({
    untilProcessExitMs: Math.round(performance.now() - startedAt),
  });
  await browser.close();
  browser = undefined;
  await expect.poll(ownedProcesses, { timeout: 10000 }).toEqual([]);
}

try {
  await start();
  await main.screenshot({
    path: resolve(evidence, "workspace.png"),
    fullPage: true,
  });
  measurements.idleProcesses = await ownedProcesses();
  await main.setViewportSize({ width: 680, height: 760 });
  expect(
    await main.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await main.screenshot({
    path: resolve(evidence, "workspace-narrow.png"),
    fullPage: true,
  });
  await main.setViewportSize({ width: 1240, height: 820 });
  await main.getByRole("button", { name: "开始设置" }).click();
  await main
    .getByLabel("本地口令", { exact: true })
    .fill("synthetic-native-password");
  await main.getByLabel("再次输入口令").fill("synthetic-native-password");
  await main.getByRole("button", { name: "创建并解锁" }).click();
  await expect(main.getByRole("dialog")).toHaveCount(0);
  checks.push("Vault created through native UI");
  await main
    .getByRole("navigation")
    .getByRole("button", { name: /Claude/ })
    .click();
  await main
    .getByRole("button", { name: "导入账号", exact: true })
    .first()
    .click();
  await main.getByLabel("账号名称").fill("合成测试账号 A");
  await main
    .getByLabel("API Key", { exact: true })
    .fill("synthetic-native-key-a");
  await main.getByRole("button", { name: "加密保存账号" }).click();
  await expect(main.getByText("合成测试账号 A", { exact: true })).toBeVisible();
  await main.getByRole("button", { name: "添加连接", exact: true }).click();
  await main.getByLabel("连接名称").fill("本地验证连接");
  await main.getByLabel("模型标识").fill("synthetic-model");
  await main.getByLabel("服务地址").fill("https://example.test/v1");
  await main.getByRole("button", { name: "保存连接", exact: true }).click();
  await expect(main.getByText("本地验证连接", { exact: true })).toBeVisible();
  await main.screenshot({
    path: resolve(evidence, "accounts.png"),
    fullPage: true,
  });
  checks.push("Native UI import and connection persistence");
  await main.getByRole("button", { name: "悬浮窗", exact: true }).click();
  const float = browser
    .contexts()[0]
    .pages()
    .find((page) => page.url().includes("float.html"));
  await expect(
    float.getByText("合成测试账号 A", { exact: true }),
  ).toBeVisible();
  const originalWindow = await nativeWindow();
  expect(originalWindow.decorated).toBe(false);
  expect(originalWindow.topmost).toBe(true);
  expect(originalWindow.alpha).toBe(209);
  expect(originalWindow.rounded).toBe(true);
  await float.locator("#tCfg").click();
  await float.locator("#roundedChip").click();
  await expect.poll(async () => (await nativeWindow()).rounded).toBe(false);
  await float.locator("#roundedChip").click();
  await expect.poll(async () => (await nativeWindow()).rounded).toBe(true);
  await float.getByRole("slider", { name: "不透明度", exact: true }).focus();
  await float.keyboard.press("Home");
  await expect.poll(async () => (await nativeWindow()).alpha).toBe(51);
  await float.locator("#tPin").click();
  await expect.poll(async () => (await nativeWindow()).topmost).toBe(false);
  await float.locator('[data-t="forest"]').click();
  await float
    .getByLabel("悬浮窗背景图片")
    .setInputFiles(resolve(root, "src/features/floating/bg-default.jpg"));
  await expect
    .poll(async () =>
      (await invoke(float, "get_float_state")).background?.startsWith(
        "data:image/jpeg;base64,",
      ),
    )
    .toBe(true);
  await float.locator("#bgClear").click();
  await expect
    .poll(async () => (await invoke(float, "get_float_state")).background)
    .toBe(null);
  await float.locator("#tCfg").click();
  const movedWindow = await nativeWindow("drag");
  expect(movedWindow.x).toBeGreaterThan(originalWindow.x);
  const resizedWindow = await nativeWindow("resize");
  expect(resizedWindow.width).toBeGreaterThan(originalWindow.width);
  await expect
    .poll(async () => (await invoke(float, "get_float_state")).width)
    .toBe(Math.round((resizedWindow.width * 96) / resizedWindow.dpi));
  measurements.floatingWindow = {
    original: originalWindow,
    moved: movedWindow,
    resized: resizedWindow,
  };
  await float.screenshot({ path: resolve(evidence, "floating-native.png") });
  checks.push(
    "Frameless native opacity, pin, background, physical drag and resize verified",
  );
  const denied = await float.evaluate(() =>
    window.__TAURI_INTERNALS__.invoke("lock_vault").then(
      () => null,
      (error) => error,
    ),
  );
  expect(denied.code).toBe("permission_denied");
  await float.getByRole("button", { name: "隐藏悬浮窗" }).click();
  await expect
    .poll(() => invoke(float, "plugin:window|is_visible", { label: "float" }))
    .toBe(false);
  checks.push("Float uses live core state and cannot mutate credentials");
  await invoke(main, "plugin:window|close", { label: "main" });
  await expect
    .poll(() => invoke(main, "plugin:window|is_visible", { label: "main" }))
    .toBe(false);
  expect(child.exitCode).toBe(null);
  await invoke(main, "check_storage");
  expect((await invoke(main, "get_snapshot")).tasks[0].state).toBe("succeeded");
  const duplicate = spawn(
    executable,
    ["--state-root", resolve(evidence, "second-state"), "--offline"],
    {
      cwd: root,
      windowsHide: true,
      stdio: ["ignore", logs, logs],
      env: process.env,
    },
  );
  const duplicateCode = await new Promise((done) =>
    duplicate.once("exit", done),
  );
  expect(duplicateCode).toBe(0);
  await expect
    .poll(() => invoke(main, "plugin:window|is_visible", { label: "main" }))
    .toBe(true);
  checks.push(
    "Close hides, hidden backend continues, second instance reopens original",
  );
  await main.getByRole("button", { name: "设置", exact: true }).click();
  await main.getByRole("checkbox", { name: /启动时显示悬浮窗/ }).click();
  await expect(
    main.getByRole("checkbox", { name: /启动时显示悬浮窗/ }),
  ).toBeChecked();
  await main.getByRole("button", { name: "运行检查", exact: true }).click();
  await expect
    .poll(async () => (await invoke(main, "get_snapshot")).tasks.length)
    .toBe(2);
  await main.screenshot({
    path: resolve(evidence, "settings.png"),
    fullPage: true,
  });
  await exit();
  checks.push("Unified native exit completed");
  await start();
  const restored = await invoke(main, "get_snapshot");
  expect(restored.accounts.length).toBe(1);
  expect(restored.connections.length).toBe(1);
  expect(restored.settings.floatEnabled).toBe(true);
  expect(restored.vaultUnlocked).toBe(false);
  const restoredFloat = await invoke(main, "get_float_state");
  expect(restoredFloat.preferences.alpha).toBe(20);
  expect(restoredFloat.preferences.theme).toBe("forest");
  expect(restoredFloat.preferences.onTop).toBe(false);
  const restoredNative = await nativeWindow();
  expect(restoredNative.alpha).toBe(51);
  expect(restoredNative.topmost).toBe(false);
  checks.push(
    "Restart restores accounts, connections, settings and task history; vault relocks",
  );
  await exit();
  expect(failures).toEqual([]);
  writeFileSync(
    resolve(evidence, "result.json"),
    JSON.stringify(
      {
        platform: process.platform,
        source:
          "native Tauri / WebView2, synthetic credentials, no service calls",
        checks,
        measurements,
        failures,
      },
      null,
      2,
    ),
  );
  console.log(`Native verification passed: ${evidence}`);
} catch (error) {
  if (main && !main.isClosed())
    await main
      .screenshot({ path: resolve(evidence, "failure.png") })
      .catch(() => {});
  writeFileSync(
    resolve(evidence, "result.json"),
    JSON.stringify({ checks, failures, error: String(error) }, null, 2),
  );
  throw error;
} finally {
  if (browser) await browser.close();
  if (child && child.exitCode === null) child.kill();
  closeSync(logs);
}
