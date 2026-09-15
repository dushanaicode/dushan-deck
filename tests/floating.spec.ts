import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { floatFixture, installFloatMock } from "./float-fixture";

test("Quota controls preserve their interactions and saved settings", async ({
  page,
}) => {
  await page.setViewportSize({ width: 430, height: 800 });
  const background =
    "data:image/jpeg;base64," +
    readFileSync("src/features/floating/bg-default.jpg").toString("base64");
  await installFloatMock(page, floatFixture(background));
  await page.goto("/float.html");
  await expect(page.locator(".card")).toHaveCount(2);
  await expect(page.locator("#hdr b")).toHaveText("Quota");
  await page.locator(".rst-toggle").first().click();
  await expect(page.locator(".rst-toggle").first()).toContainText("重置于");
  await page.locator("#tCfg").click();
  await page.locator('[data-t="paper"]').click();
  await expect(page.locator("body")).toHaveAttribute("data-theme", "paper");
  await page.getByRole("slider", { name: "不透明度", exact: true }).focus();
  await page.keyboard.press("Home");
  await expect(
    page.getByRole("slider", { name: "不透明度", exact: true }),
  ).toHaveValue("20");
  await page.locator('[data-n="#usage"]').click();
  await page.locator("#tCfg").click();
  await expect(page.locator(".usage")).toHaveCount(0);
  const from = await page.locator(".name").first().boundingBox();
  const to = await page.locator(".name").last().boundingBox();
  await page.mouse.move(from!.x + 30, from!.y + 8);
  await page.mouse.down();
  await page.mouse.move(to!.x + 30, to!.y + 60, { steps: 8 });
  await page.mouse.up();
  await expect(page.locator(".card").first()).toHaveAttribute(
    "data-key",
    "openai:beta",
  );
  await page.locator("#tCfg").click();
  await page.locator("#orderReset").click();
  await page.locator("#tCfg").click();
  await expect(page.locator(".card").first()).toHaveAttribute(
    "data-key",
    "claude:alpha",
  );
  await page.locator("#tCfg").click();
  await page.locator("#animationChip").click();
  await expect(page.locator("#animationChip")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page.locator("#tCfg").click();
  await page.locator("#tRef").click();
  await expect(page.locator("#panel")).toHaveClass(/refreshed/);
  await page.evaluate(() => {
    (
      window as unknown as { __floatFixture: { failRefresh: boolean } }
    ).__floatFixture.failRefresh = true;
  });
  await page.locator("#tRef").click();
  await expect(page.locator("#status")).toContainText("合成网络错误");
  await expect(page.locator(".card")).toHaveCount(2);
  await page.locator("#tQuit").click();
  const saved = await page.evaluate(
    () =>
      (
        window as unknown as {
          __floatFixture: {
            state: { preferences: { alpha: number; theme: string } };
            calls: string[];
          };
        }
      ).__floatFixture,
  );
  expect(saved.state.preferences.alpha).toBe(20);
  expect(saved.state.preferences.theme).toBe("paper");
  expect(saved.calls).toContain("plugin:window|close");
});
