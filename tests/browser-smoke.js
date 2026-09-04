"use strict";

const assert = require("node:assert/strict");
const childProcess = require("node:child_process");
const fs = require("node:fs");
const net = require("node:net");
const os = require("node:os");
const path = require("node:path");

const debugEnabled = process.env.MINIMODEL_BROWSER_DEBUG === "1";

function debug(message) {
  if (debugEnabled) {
    process.stderr.write(`[browser-smoke] ${message}\n`);
  }
}

function findChrome() {
  const candidates = [
    process.env.CHROME_PATH,
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe"
  ].filter(Boolean);
  const browser = candidates.find((candidate) => fs.existsSync(candidate));
  if (!browser) {
    throw new Error("Chrome or Edge was not found; set CHROME_PATH to run the browser smoke test");
  }
  return browser;
}

async function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close((error) => error ? reject(error) : resolve(address.port));
    });
  });
}

async function waitForTarget(port, timeoutMs = 15000) {
  const deadline = Date.now() + timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json`);
      const targets = await response.json();
      const page = targets.find((target) => target.type === "page");
      if (page && page.webSocketDebuggerUrl) {
        return page;
      }
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`browser debugging target did not start${lastError ? `: ${lastError.message}` : ""}`);
}

function cdpClient(socket) {
  let nextId = 1;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (!message.id || !pending.has(message.id)) {
      return;
    }
    const request = pending.get(message.id);
    pending.delete(message.id);
    clearTimeout(request.timeout);
    if (message.error) {
      request.reject(new Error(message.error.message || JSON.stringify(message.error)));
    } else {
      request.resolve(message.result);
    }
  });
  return {
    send(method, params = {}) {
      const id = nextId++;
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          pending.delete(id);
          reject(new Error(`CDP ${method} timed out`));
        }, 15000);
        pending.set(id, { resolve, reject, timeout });
        socket.send(JSON.stringify({ id, method, params }));
      });
    }
  };
}

async function waitForPageReady(client, timeoutMs = 15000) {
  const deadline = Date.now() + timeoutMs;
  let lastState = "";
  while (Date.now() < deadline) {
    const result = await client.send("Runtime.evaluate", {
      expression: `JSON.stringify({
        readyState: document.readyState,
        status: document.getElementById("modelCatalogStatus")?.dataset.state || "",
        catalogSettled: Boolean(document.querySelector("#modelCatalogList .catalog-model, #modelCatalogList .catalog-empty"))
      })`,
      returnByValue: true
    });
    lastState = result.result && result.result.value || "";
    const state = JSON.parse(lastState || "{}");
    if (state.readyState === "complete" && state.catalogSettled && (state.status === "ready" || state.status === "warning")) {
      return state;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`catalog page did not become ready: ${lastState}`);
}

async function inspectViewport(client, width, height, url) {
  await client.send("Emulation.setDeviceMetricsOverride", {
    width,
    height,
    deviceScaleFactor: 1,
    mobile: width <= 620
  });
  await client.send("Page.navigate", { url });
  await waitForPageReady(client);
  const initialResult = await client.send("Runtime.evaluate", {
    expression: `JSON.stringify({
      selectedAvailability: document.getElementById("modelCatalogAvailability")?.value || "",
      modelCount: document.querySelectorAll("#modelCatalogList .catalog-model").length,
      emptyText: document.querySelector("#modelCatalogList .catalog-empty")?.textContent.trim() || "",
      actionCount: document.querySelectorAll("#modelCatalogList a, #modelCatalogList button").length
    })`,
    returnByValue: true
  });
  const initial = JSON.parse(initialResult.result.value);
  await client.send("Runtime.evaluate", {
    expression: `(() => {
      const select = document.getElementById("modelCatalogAvailability");
      select.value = "all";
      select.dispatchEvent(new Event("change", { bubbles: true }));
    })()`
  });
  const evaluated = await client.send("Runtime.evaluate", {
    expression: `(() => {
      window.scrollTo(0, 0);
      const rect = (selector) => {
        const node = document.querySelector(selector);
        if (!node) return null;
        const value = node.getBoundingClientRect();
        return { left: value.left, right: value.right, top: value.top, bottom: value.bottom, width: value.width, height: value.height };
      };
      const model = document.querySelector("#modelCatalogList .catalog-model");
      return JSON.stringify({
        viewport: { width: innerWidth, height: innerHeight },
        initial: ${JSON.stringify(initial)},
        documentWidth: document.documentElement.scrollWidth,
        bodyWidth: document.body.scrollWidth,
        header: rect(".site-header"),
        hero: rect(".hero"),
        heroHeading: rect(".hero h1"),
        heroActions: Array.from(document.querySelectorAll(".hero .button"), (node) => {
          const value = node.getBoundingClientRect();
          return { text: node.textContent.trim(), left: value.left, right: value.right };
        }),
        catalog: rect("#models"),
        catalogHeading: rect("#modelCatalogHeading"),
        model: rect("#modelCatalogList .catalog-model"),
        modelCount: document.querySelectorAll("#modelCatalogList .catalog-model").length,
        modelText: document.getElementById("modelCatalogList")?.textContent.replace(/\\s+/g, " ").trim() || "",
        totals: document.querySelector(".catalog-totals")?.textContent.replace(/\\s+/g, " ").trim() || "",
        status: document.getElementById("modelCatalogStatus")?.textContent || "",
        memoryHeading: Array.from(document.querySelectorAll("h2")).find((node) => node.textContent.includes("MemoryEndpoints"))?.textContent.trim() || "",
        memoryText: Array.from(document.querySelectorAll("section")).find((node) => node.querySelector("h2")?.textContent.includes("MemoryEndpoints"))?.textContent.replace(/\\s+/g, " ").trim() || "",
        releaseText: document.querySelector("footer")?.textContent.replace(/\\s+/g, " ").trim() || "",
        pageText: document.body.textContent.replace(/\\s+/g, " ").trim()
      });
    })()`,
    returnByValue: true
  });
  return JSON.parse(evaluated.result.value);
}

async function capture(client, outputPath, scrollToCatalog) {
  if (scrollToCatalog) {
    await client.send("Runtime.evaluate", {
      expression: `document.getElementById("models").scrollIntoView({ block: "start", behavior: "instant" })`
    });
  }
  const screenshot = await client.send("Page.captureScreenshot", {
    format: "png",
    captureBeyondViewport: false,
    fromSurface: true
  });
  fs.writeFileSync(outputPath, Buffer.from(screenshot.data, "base64"));
}

function stopBrowserTree(profile, browserProcess) {
  if (process.platform === "win32") {
    const script = [
      `$profile = '${profile.replace(/'/g, "''")}'`,
      "$targets = Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -and $_.CommandLine.Contains($profile) }",
      "$targets | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }"
    ].join("; ");
    const encoded = Buffer.from(script, "utf16le").toString("base64");
    childProcess.spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-EncodedCommand", encoded], {
      stdio: "ignore",
      windowsHide: true,
      timeout: 10000
    });
    return;
  }
  if (browserProcess.exitCode === null) {
    browserProcess.kill("SIGKILL");
  }
}

function assertViewport(result, label) {
  assert.equal(result.initial.selectedAvailability, "all", `${label}: all qualified compositions was not the default`);
  assert.equal(result.initial.modelCount, 0, `${label}: an unqualified composition appeared in the catalog`);
  assert.equal(result.initial.actionCount, 0, `${label}: an acquisition action appeared without a qualified composition`);
  assert.match(result.initial.emptyText, /no SLM2 composition is qualified yet/i);
  assert.equal(result.modelCount, 0, `${label}: the all-qualified view was not empty`);
  assert.ok(result.documentWidth <= result.viewport.width + 1, `${label}: document overflowed horizontally (${result.documentWidth} > ${result.viewport.width})`);
  assert.ok(result.bodyWidth <= result.viewport.width + 1, `${label}: body overflowed horizontally (${result.bodyWidth} > ${result.viewport.width})`);
  assert.ok(result.catalog && result.catalog.top < result.viewport.height, `${label}: catalog was not visible in the initial viewport`);
  assert.ok(result.heroHeading && result.heroHeading.left >= 0 && result.heroHeading.right <= result.viewport.width + 1, `${label}: hero heading was off-canvas`);
  for (const action of result.heroActions) {
    assert.ok(action.left >= 0 && action.right <= result.viewport.width + 1, `${label}: ${action.text} action was off-canvas`);
  }
  assert.match(result.modelText, /no SLM2 composition is qualified yet/i);
  assert.match(result.totals, /0 qualified compositions/);
  assert.match(result.totals, /0 with current direct peers/);
  assert.match(result.totals, /0 pinned conversion routes/);
  assert.match(result.status, /zero compositions are qualified/i);
  assert.equal(result.memoryHeading, "MemoryEndpoints has two separate uses");
  assert.match(result.memoryText, /ecosystem agents use MemoryEndpoints\.com directly/i);
  assert.match(result.memoryText, /TinyRustLM\.com Connect user can sign in through its plugin/i);
  assert.match(result.releaseText, /Version 1\.1\.2 \| Prepared 2026-08-31 \| Last published 1\.1\.1 on 2026-08-29/);
  assert.doesNotMatch(result.pageText, /Qwen2\.5|manifest-v0|models\.v0|download (?:a )?(?:request|script)/i);
}

async function main() {
  const url = process.argv[2] || "http://127.0.0.1:8191/";
  const browser = findChrome();
  const port = await freePort();
  const profile = path.join(os.tmpdir(), `minimodel-browser-${process.pid}-${Date.now()}`);
  const desktopScreenshot = path.join(os.tmpdir(), `minimodel-desktop-${process.pid}.png`);
  const mobileScreenshot = path.join(os.tmpdir(), `minimodel-mobile-${process.pid}.png`);
  await fs.promises.rm(profile, { recursive: true, force: true });
  const browserProcess = childProcess.spawn(browser, [
    "--headless=new",
    "--disable-gpu",
    "--disable-background-networking",
    "--disable-component-update",
    "--disable-extensions",
    "--no-default-browser-check",
    "--no-first-run",
    `--remote-debugging-port=${port}`,
    `--user-data-dir=${profile}`,
    "about:blank"
  ], { stdio: "ignore", windowsHide: true });

  let socket;
  try {
    debug("waiting for browser target");
    const target = await waitForTarget(port);
    debug("connecting to CDP");
    socket = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((resolve, reject) => {
      socket.addEventListener("open", resolve, { once: true });
      socket.addEventListener("error", reject, { once: true });
    });
    const client = cdpClient(socket);
    await client.send("Page.enable");
    await client.send("Runtime.enable");

    debug("inspecting desktop");
    const desktop = await inspectViewport(client, 1440, 1000, url);
    assertViewport(desktop, "desktop");
    debug("capturing desktop");
    await capture(client, desktopScreenshot, false);

    debug("inspecting mobile");
    const mobile = await inspectViewport(client, 390, 844, url);
    assertViewport(mobile, "mobile");
    debug("capturing mobile catalog");
    await capture(client, mobileScreenshot, true);

    process.stdout.write(`${JSON.stringify({ desktop, mobile, desktopScreenshot, mobileScreenshot }, null, 2)}\n`);
  } finally {
    debug("stopping browser");
    stopBrowserTree(profile, browserProcess);
    if (socket && socket.readyState !== WebSocket.CLOSED) {
      socket.close();
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
    try {
      await fs.promises.rm(profile, { recursive: true, force: true, maxRetries: 12, retryDelay: 100 });
    } catch (error) {
      debug(`profile cleanup deferred: ${error.message}`);
    }
    debug("browser stopped");
  }
}

main().catch((error) => {
  console.error(error.stack || error);
  process.exitCode = 1;
});
