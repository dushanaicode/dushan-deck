import { chromium } from "@playwright/test";
import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  floatFixture,
  originalFixture,
  installFloatMock,
  fixtureTime,
} from "../tests/float-fixture.ts";

const output = resolve("Temp/quota-float-20260915/visual");
mkdirSync(output, { recursive: true });
const reference = readFileSync(
  "Temp/references/dushan-quota/lib/assets/float.html",
  "utf8",
);
const background =
  "data:image/jpeg;base64," +
  readFileSync("src/features/floating/bg-default.jpg").toString("base64");
const fixture = floatFixture(background);
const browser = await chromium.launch({ channel: "msedge", headless: true });
const cases = [
  { name: "compact", width: 290, height: 430, settings: false, theme: "dark" },
  { name: "wide", width: 430, height: 800, settings: false, theme: "dark" },
  { name: "settings", width: 290, height: 700, settings: true, theme: "dark" },
  { name: "paper", width: 290, height: 700, settings: true, theme: "paper" },
  { name: "ocean", width: 290, height: 700, settings: true, theme: "ocean" },
  { name: "forest", width: 290, height: 700, settings: true, theme: "forest" },
  { name: "violet", width: 290, height: 700, settings: true, theme: "violet" },
  { name: "empty", width: 290, height: 430, settings: false, theme: "dark" },
  { name: "stale", width: 290, height: 700, settings: false, theme: "dark" },
];
const results = [];
try {
  for (const item of cases) {
    const current = structuredClone(fixture);
    if (item.name === "empty") current.snapshot.results = [];
    if (item.name === "stale") {
      current.snapshot.state = "stale";
      current.snapshot.results[0].notice = "服务限流";
      current.snapshot.results[0].retryAt =
        Date.parse(fixtureTime) / 1000 + 120;
    }
    const context = await browser.newContext({
      viewport: { width: item.width, height: item.height },
      deviceScaleFactor: 1,
      timezoneId: "UTC",
    });
    const before = await context.newPage(),
      after = await context.newPage();
    await before.clock.setFixedTime(new Date(fixtureTime));
    await before.route("**/reference.html", (route) =>
      route.fulfill({ contentType: "text/html", body: reference }),
    );
    await before.addInitScript(({ settings, payload, background }) => {
      Object.assign(window, {
        pywebview: {
          api: {
            settings: async () => settings,
            quota: async () => payload,
            background: async () => ({ data_url: background }),
            save_settings: async () => ({ ok: true }),
            set_alpha: async () => ({ ok: true }),
            set_rounded: async () => ({ ok: true }),
            toggle_on_top: async () => ({ on_top: false }),
            open_web: async () => {},
            quit: async () => {},
            begin_drag: async () => {},
            begin_resize: async () => {},
          },
        },
      });
    }, originalFixture(current));
    await installFloatMock(after, current);
    await before.goto("http://127.0.0.1:1420/reference.html");
    await after.goto("http://127.0.0.1:1420/float.html");
    for (const page of [before, after]) {
      if (item.name === "empty")
        await page.waitForFunction(
          () => document.getElementById("list")?.textContent === "没有账号",
        );
      else await page.locator(".card").last().waitFor();
      await page.waitForFunction(() => document.body.classList.contains("bg"));
      await page.evaluate(async (background) => {
        const image = new Image();
        image.src = background;
        await image.decode();
        await document.fonts.ready;
      }, background);
      if (item.settings) await page.locator("#tCfg").click();
      if (item.theme !== "dark")
        await page.locator(`[data-t="${item.theme}"]`).click();
      await page.mouse.move(0, 0);
    }
    const a = await before.screenshot({
        path: resolve(output, `${item.name}-reference.png`),
      }),
      b = await after.screenshot({
        path: resolve(output, `${item.name}-deck.png`),
      });
    const compared = await before.evaluate(
      async ([a, b]) => {
        const load = async (value) => {
          const image = new Image();
          image.src = "data:image/png;base64," + value;
          await image.decode();
          return image;
        };
        const [left, right] = await Promise.all([load(a), load(b)]);
        const canvas = document.createElement("canvas");
        canvas.width = left.width * 2;
        canvas.height = left.height;
        const ctx = canvas.getContext("2d");
        ctx.drawImage(left, 0, 0);
        ctx.drawImage(right, left.width, 0);
        const first = ctx.getImageData(0, 0, left.width, left.height).data,
          second = ctx.getImageData(
            left.width,
            0,
            left.width,
            left.height,
          ).data;
        let pixels = 0,
          max = 0;
        const differences = [];
        for (let i = 0; i < first.length; i += 4) {
          let difference = 0;
          for (let j = 0; j < 4; j++)
            difference = Math.max(
              difference,
              Math.abs(first[i + j] - second[i + j]),
            );
          if (difference) {
            pixels++;
            max = Math.max(max, difference);
            if (differences.length < 40)
              differences.push({
                x: (i / 4) % left.width,
                y: Math.floor(i / 4 / left.width),
                delta: difference,
              });
          }
        }
        return {
          pixels,
          max,
          differences,
          total: left.width * left.height,
          comparison: canvas.toDataURL("image/png"),
        };
      },
      [a.toString("base64"), b.toString("base64")],
    );
    writeFileSync(
      resolve(output, `${item.name}-comparison.png`),
      Buffer.from(compared.comparison.split(",")[1], "base64"),
    );
    results.push({
      name: item.name,
      viewport: [item.width, item.height],
      differentPixels: compared.pixels,
      totalPixels: compared.total,
      maxChannelDifference: compared.max,
      differences: compared.differences,
    });
    await context.close();
  }
  writeFileSync(
    resolve(output, "comparison.json"),
    JSON.stringify(results, null, 2),
  );
  console.log(JSON.stringify(results, null, 2));
} finally {
  await browser.close();
}
