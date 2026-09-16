import { spawn, spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { createServer as createNetServer } from "node:net";
import { resolve } from "node:path";

const root = process.cwd();
if (!existsSync(resolve(root, "scripts/deck.mjs")))
  throw new Error("Run from the Dushan Deck project root.");
const temp = resolve(root, "Temp/tooling");
mkdirSync(temp, { recursive: true });
const tauriGen = resolve(root, "Temp/build/tauri-gen");
mkdirSync(tauriGen, { recursive: true });
if (!existsSync(resolve(root, "src-tauri/gen")))
  symlinkSync(
    tauriGen,
    resolve(root, "src-tauri/gen"),
    process.platform === "win32" ? "junction" : "dir",
  );
if (realpathSync(resolve(root, "src-tauri/gen")) !== realpathSync(tauriGen))
  throw new Error("src-tauri/gen must point to Temp/build/tauri-gen.");
if (
  existsSync(resolve(root, "node_modules")) &&
  realpathSync(resolve(root, "node_modules")) !==
    realpathSync(resolve(temp, "frontend/node_modules"))
)
  throw new Error(
    "node_modules must point to Temp/tooling/frontend/node_modules.",
  );
Object.assign(process.env, {
  TEMP: temp,
  TMP: temp,
  TMPDIR: temp,
  CARGO_HOME: resolve(temp, "cargo-home"),
  CARGO_TARGET_DIR: resolve(root, "Temp/build/rust"),
  npm_config_cache: resolve(temp, "npm-cache"),
  npm_config_update_notifier: "false",
  npm_config_audit: "false",
  npm_config_fund: "false",
  PLAYWRIGHT_BROWSERS_PATH: resolve(temp, "browsers"),
  RUSTUP_AUTO_INSTALL: "0",
  DECK_TEST_ROOT: resolve(root, "Temp/verification"),
});

async function run(command, args) {
  const child = spawn(command, args, {
    cwd: root,
    stdio: "inherit",
    env: process.env,
    shell: false,
  });
  const code = await new Promise((done, reject) => {
    child.on("error", reject);
    child.on("exit", done);
  });
  if (code !== 0) throw new Error(`${command} exited with ${code}`);
}
async function nodeTool(relative, args = []) {
  await run(process.execPath, [
    resolve(root, "node_modules", relative),
    ...args,
  ]);
}
async function icons() {
  const sourceTime = statSync(resolve(root, "src/assets/deck.svg")).mtimeMs;
  if (
    ["icon.ico", "icon.png"].every((name) => {
      const output = resolve(root, "Temp/build/icons", name);
      return existsSync(output) && statSync(output).mtimeMs >= sourceTime;
    })
  )
    return;
  await nodeTool("@tauri-apps/cli/tauri.js", [
    "icon",
    "src/assets/deck.svg",
    "--output",
    "Temp/build/icons",
  ]);
}

function reuseDesktop() {
  if (process.platform !== "win32") return false;
  const result = spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-File",
      resolve(root, "scripts/reuse-dev.ps1"),
      "-Executable",
      resolve(root, "Temp/build/rust/debug/dushan-deck.exe"),
      "-StateRoot",
      resolve(root, "Temp/dev-state"),
    ],
    { cwd: root, env: process.env, stdio: "inherit", windowsHide: true },
  );
  if (result.error) throw result.error;
  if (result.status === 10) return false;
  if (result.status !== 0)
    throw new Error(`Desktop activation exited with ${result.status}`);
  console.log("Dushan Deck 已在运行，已唤回主窗口。");
  return true;
}

async function devPortAvailable() {
  const probe = createNetServer();
  return new Promise((done, reject) => {
    probe.once("error", (error) => {
      if (error.code === "EADDRINUSE") done(false);
      else reject(error);
    });
    probe.listen(1420, "127.0.0.1", () => probe.close(() => done(true)));
  });
}

switch (process.argv[2]) {
  case "icons":
    await icons();
    break;
  case "install": {
    const prefix = resolve(temp, "frontend");
    mkdirSync(prefix, { recursive: true });
    const manifest = JSON.parse(
      readFileSync(resolve(root, "package.json"), "utf8"),
    );
    delete manifest.scripts;
    writeFileSync(
      resolve(prefix, "package.json"),
      JSON.stringify(manifest, null, 2) + "\n",
    );
    if (existsSync(resolve(root, "package-lock.json")))
      copyFileSync(
        resolve(root, "package-lock.json"),
        resolve(prefix, "package-lock.json"),
      );
    // npm's CLI lives beside node on Windows, and beside its resolved launcher on Unix.
    const npmCli =
      process.platform === "win32"
        ? resolve(process.execPath, "..", "node_modules/npm/bin/npm-cli.js")
        : realpathSync(resolve(process.execPath, "..", "npm"));
    await run(process.execPath, [
      npmCli,
      existsSync(resolve(root, "package-lock.json")) ? "ci" : "install",
      "--prefix",
      prefix,
      "--ignore-scripts",
    ]);
    copyFileSync(
      resolve(prefix, "package-lock.json"),
      resolve(root, "package-lock.json"),
    );
    if (!existsSync(resolve(root, "node_modules")))
      symlinkSync(
        resolve(prefix, "node_modules"),
        resolve(root, "node_modules"),
        process.platform === "win32" ? "junction" : "dir",
      );
    break;
  }
  case "web":
    {
      const { createServer } = await import("vite");
      const server = await createServer({ configLoader: "runner" });
      await server.listen();
      server.printUrls();
    }
    break;
  case "frontend":
    await nodeTool("typescript/bin/tsc", ["--noEmit"]);
    await nodeTool("vite/bin/vite.js", ["build", "--configLoader", "runner"]);
    break;
  case "dev":
    if (reuseDesktop()) break;
    if (!(await devPortAvailable())) {
      console.error(
        "端口 1420 已被占用。若 Dushan Deck 正在启动，请稍候重试；否则请先退出占用该端口的开发服务。",
      );
      process.exitCode = 1;
      break;
    }
    await icons();
    await nodeTool("@tauri-apps/cli/tauri.js", [
      "dev",
      "--",
      "--",
      "--state-root",
      resolve(root, "Temp/dev-state"),
    ]);
    break;
  case "build":
    await icons();
    await nodeTool("@tauri-apps/cli/tauri.js", [
      "build",
      "--debug",
      "--no-bundle",
    ]);
    break;
  case "check":
    await icons();
    await nodeTool("typescript/bin/tsc", ["--noEmit"]);
    await nodeTool("prettier/bin/prettier.cjs", [
      "--check",
      "src",
      "scripts",
      "tests",
      "*.json",
      "*.ts",
      "*.html",
      "README.md",
      "src-tauri/*.json",
    ]);
    await run("cargo", ["fmt", "--all", "--check"]);
    await run("cargo", [
      "clippy",
      "--workspace",
      "--all-targets",
      "--locked",
      "--",
      "-D",
      "warnings",
    ]);
    break;
  case "format":
    await nodeTool("prettier/bin/prettier.cjs", [
      "--write",
      "src",
      "scripts",
      "tests",
      "*.json",
      "*.ts",
      "*.html",
      "README.md",
      "src-tauri/*.json",
    ]);
    await run("cargo", ["fmt", "--all"]);
    break;
  case "test":
    await icons();
    await run("cargo", ["test", "--workspace", "--locked"]);
    await nodeTool("@playwright/test/cli.js", ["test"]);
    break;
  default:
    throw new Error(
      "Use install, dev, build, check, test, format, frontend or web.",
    );
}
