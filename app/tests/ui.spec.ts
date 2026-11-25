import { test, expect } from "@playwright/test";

function nav(page: import("@playwright/test").Page, name: string) {
  return page.locator("nav.rail").getByRole("button", { name, exact: true });
}

test.beforeEach(async ({ page }) => {
  await page.goto("/?mock=1", { waitUntil: "load" });
  await page.waitForTimeout(1500);
});

test("shell landmarks and nav", async ({ page }) => {
  await expect(page.locator("header.topbar")).toBeVisible();
  await expect(page.locator("nav.rail")).toBeVisible();
  await expect(page.locator("footer.statusbar")).toBeVisible();
  await expect(page.getByRole("link", { name: "Skip to content" })).toHaveCount(1);
  await expect(page.locator('.rail-item[aria-current="page"]')).toHaveText(/Console/);
  await expect(page.getByRole("heading", { name: "Living room PS4" })).toBeVisible();
  await expect(page.getByText("GoldHEN FTP server ready").first()).toBeVisible();
});

test("library page lists installed titles and packages", async ({ page }) => {
  await nav(page, "Library").click();
  await expect(page.locator(".tile")).toHaveCount(6);
  await expect(page.getByText("God of War", { exact: true }).first()).toBeVisible();
  await expect(page.locator(".tile .t").filter({ hasText: "update" })).toBeVisible();
});

test("library filters games, homebrew and packages", async ({ page }) => {
  await nav(page, "Library").click();
  await page.getByRole("tab", { name: "Games" }).click();
  await expect(page.locator(".tile")).toHaveCount(3);
  await page.getByRole("tab", { name: "Homebrew" }).click();
  await expect(page.locator(".tile")).toHaveCount(1);
  await expect(page.getByText("Control4Free").first()).toBeVisible();
  await page.getByRole("tab", { name: "Packages" }).click();
  await expect(page.locator(".tile")).toHaveCount(2);
});

test("downloads page lists local files", async ({ page }) => {
  await nav(page, "Downloads").click();
  await expect(page.locator("#workspace").getByText("Control4Free-1.3.1.pkg")).toBeVisible();
});

test("homebrew catalog renders with install buttons", async ({ page }) => {
  await nav(page, "Homebrew").click();
  await expect(page.locator(".store-card")).toHaveCount(2);
  await expect(page.locator(".store-card .name").filter({ hasText: "Control4Free" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Install" }).first()).toBeVisible();
});

test("controller page offers the pad and the service state", async ({ page }) => {
  await nav(page, "Controller").click();
  await expect(page.getByRole("heading", { name: "Virtual DualShock 4" })).toBeVisible();
  await expect(page.getByText("service ready")).toBeVisible();
  await expect(page.getByRole("button", { name: "X", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "PS" })).toBeVisible();
  await expect(page.getByRole("slider", { name: "L2" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Connect" })).toBeVisible();
});

test("saves page lists the save folders", async ({ page }) => {
  await nav(page, "Saves").click();
  await expect(page.locator(".row")).toHaveCount(2);
  await expect(page.getByText("Bloodborne").first()).toBeVisible();
});

test("conversions page explains the tools", async ({ page }) => {
  await nav(page, "Convert").click();
  await expect(page.getByText("Unpack a PKG", { exact: true })).toBeVisible();
  await expect(page.getByText("Merge parts")).toBeVisible();
});

test("inspector queue shows the seeded job", async ({ page }) => {
  await expect(page.locator(".queue-card .job-row")).toHaveCount(1);
});

test("settings exposes config controls", async ({ page }) => {
  await nav(page, "Settings").click();
  await expect(page.getByText("Game search backend").first()).toBeVisible();
  await expect(page.getByText("FAT32 split size (MiB)")).toBeVisible();
});

test("unslop-anything a11y pass: search inputs have accessible names", async ({ page }) => {
  await page.getByRole("button", { name: "Library", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "Filter by title" })).toBeVisible();
  await page.getByRole("button", { name: "Game Index" }).click();
  await expect(page.getByRole("textbox", { name: "Search games" })).toBeVisible();
});
