import { expect, test } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";

test("development server excludes reference code, documents and application data", async ({
  request,
}) => {
  mkdirSync("Temp/verification/dev-boundary", { recursive: true });
  writeFileSync(
    "Temp/verification/dev-boundary/deck.sqlite3",
    "synthetic-private-marker",
  );
  for (const path of [
    "/Temp/verification/dev-boundary/deck.sqlite3",
    "/Temp/references/dushan-quota/lib/models.py",
    "/.docs/README.md",
  ]) {
    const response = await request.get(path);
    expect(response.status()).toBe(403);
    expect(await response.text()).not.toContain("synthetic-private-marker");
  }
});

test("a browser without the desktop core never invents successful data", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByRole("alert")).toContainText(
    "此页面需要 Dushan Deck 桌面应用",
  );
  await expect(
    page.getByRole("button", { name: "悬浮窗", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "退出应用", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "导入账号", exact: true }),
  ).toHaveCount(0);
  await page.setViewportSize({ width: 680, height: 760 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
