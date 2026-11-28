// The browser side of `cargo xtask shoot` (docs/workflow.md, decision 0240). It reads the plan the
// task wrote (argument 1), drives a headless Chromium to every shot of it and prints each picture
// as base64 between markers, since a farm job's log is the only channel back:
//
//   =====SHOT <name> <bytes> <width>x<height>=====
//   <base64, 100 columns>
//   =====END <name>=====
//
// Every other line it prints starts with `shoot:`. A browser that cannot start, a page that does
// not answer 200 and a picture that cannot be made are errors that name what failed; none is
// swallowed.

import { createRequire } from "node:module";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright-core");
const UPNG = require("upng-js");

const plan = JSON.parse(readFileSync(process.argv[2], "utf8"));
const TYPES = {
  ".html": "text/html; charset=utf-8", ".css": "text/css", ".js": "text/javascript", ".mjs": "text/javascript",
  ".svg": "image/svg+xml", ".png": "image/png", ".wasm": "application/wasm", ".ttf": "font/ttf",
  ".woff2": "font/woff2", ".woff": "font/woff", ".json": "application/json",
};

/** A server of one folder on loopback, for the mock: files only, no listing, no escape from the root. */
class Folder {
  static start({ root, port }) {
    const server = createServer((request, response) => {
      const path = normalize(decodeURIComponent(new URL(request.url, "http://x").pathname)).replace(/^(\.\.[/\\])+/, "");
      try {
        const bytes = readFileSync(join(root, path));
        response.writeHead(200, { "content-type": TYPES[extname(path)] ?? "application/octet-stream" });
        response.end(bytes);
      } catch (error) {
        response.writeHead(404, { "content-type": "text/plain" });
        response.end(`not found: ${path} (${error.code})`);
      }
    });
    return new Promise((done) => server.listen(port, "127.0.0.1", () => done(server)));
  }
}

const say = (text) => console.log(`shoot: ${text}`);

/** The most bytes of one picture printed in the log, before the palette gets smaller. */
const BUDGET = 500_000;
/** The tallest picture, in pixels: a longer page is cut, which keeps the log small. */
const TALLEST = 9_000;

class Engines {
  /** The ways to start a Chromium, in order: what each is and how it launches. */
  static async list() {
    return [
      { name: "playwright", launch: () => chromium.launch({ headless: true }) },
      {
        name: "sparticuz",
        launch: async () => {
          process.env.AWS_EXECUTION_ENV ??= "AWS_Lambda_nodejs22.x";
          const sparticuz = (await import("@sparticuz/chromium")).default;
          return chromium.launch({
            headless: true,
            executablePath: await sparticuz.executablePath(),
            // its Lambda flags run the browser in one process, which a closed context takes down
            args: sparticuz.args.filter((arg) => arg !== "--single-process" && arg !== "--no-zygote"),
          });
        },
      },
    ];
  }

  /** The first engine that starts, or an error with every cause. */
  static async start() {
    const causes = [];
    for (const engine of await Engines.list()) {
      try {
        const browser = await engine.launch();
        say(`engine ${engine.name} started, chromium ${browser.version()}`);
        return browser;
      } catch (error) {
        say(`engine ${engine.name} failed: ${String(error.message).split("\n").slice(0, 12).join(" | ")}`);
        causes.push(`${engine.name}: ${error.message}`);
      }
    }
    throw new Error(`no browser starts on this host:\n${causes.join("\n")}`);
  }
}

class Picture {
  /** A palette PNG of `png`, with the fewest colours that keep it under the budget. */
  static pack(png) {
    const image = UPNG.decode(png);
    const rgba = UPNG.toRGBA8(image)[0];
    let best = null;
    for (const colours of [256, 128, 64, 32, 16]) {
      best = { bytes: Buffer.from(UPNG.encode([rgba], image.width, image.height, colours)), colours };
      if (best.bytes.length <= BUDGET) break;
    }
    return { ...best, width: image.width, height: image.height };
  }

  /** Prints it between markers. */
  static print(name, picture) {
    console.log(`=====SHOT ${name} ${picture.bytes.length} ${picture.width}x${picture.height}=====`);
    const text = picture.bytes.toString("base64");
    for (let at = 0; at < text.length; at += 100) console.log(text.slice(at, at + 100));
    console.log(`=====END ${name}=====`);
  }
}

class Page {
  constructor(browser, shot, width) {
    this.browser = browser;
    this.shot = shot;
    this.width = width;
    this.name = `${shot.name}-${width}`;
  }

  /** Loads the page and returns what the browser said about it. */
  async open() {
    const session = this.shot.login ? SESSIONS.get(this.shot.login) : undefined;
    if (this.shot.login && session === undefined) throw new Error(`no sign-in named ${this.shot.login}`);
    this.context = await this.browser.newContext({
      viewport: { width: this.width, height: 900 },
      deviceScaleFactor: 1,
      reducedMotion: "reduce",
      storageState: session,
    });
    this.page = await this.context.newPage();
    this.notes = [];
    this.page.on("console", (message) => {
      if (["error", "warning"].includes(message.type())) this.notes.push(`console ${message.type()}: ${message.text()}`);
    });
    this.page.on("pageerror", (error) => this.notes.push(`page error: ${error.message}`));
    this.page.on("requestfailed", (request) => this.notes.push(`request failed: ${request.url()} ${request.failure()?.errorText}`));
    this.page.on("response", (response) => {
      if (response.status() >= 400) this.notes.push(`status ${response.status()}: ${response.url()}`);
    });
    const response = await this.page.goto(this.shot.url, { waitUntil: "load" });
    const wanted = this.shot.status ?? 200;
    if (response !== null && response.status() !== wanted) {
      throw new Error(`${this.shot.url} answered ${response.status()}, not ${wanted}`);
    }
    await this.page.evaluate(() => document.fonts.ready);
  }

  /** What the page measures: fonts, overflow, the widest element past the viewport. */
  async measure() {
    const found = await this.page.evaluate(() => {
      const faces = [...document.fonts].map((face) => `${face.family.replaceAll('"', "")} ${face.weight} ${face.status}`);
      const wide = document.documentElement.scrollWidth;
      const past = [...document.querySelectorAll("body *")]
        .filter((el) => el.getBoundingClientRect().right > window.innerWidth + 1)
        .slice(0, 6)
        .map((el) => `${el.tagName.toLowerCase()}.${String(el.className).split(" ")[0]} right=${Math.round(el.getBoundingClientRect().right)}`);
      return { faces, wide, inner: window.innerWidth, past, height: document.documentElement.scrollHeight };
    });
    say(`${this.name}: ${found.height}px tall, scrollWidth ${found.wide} in ${found.inner}${found.wide > found.inner ? " OVERFLOW" : ""}`);
    say(`${this.name}: faces ${found.faces.join("; ") || "none declared"}`);
    if (found.past.length > 0) say(`${this.name}: past the edge ${found.past.join("; ")}`);
    this.notes.forEach((note) => say(`${this.name}: ${note}`));
  }

  /** The picture of the whole page. */
  async take() {
    const height = await this.page.evaluate(() => document.documentElement.scrollHeight);
    const clip = height > TALLEST ? { x: 0, y: 0, width: this.width, height: TALLEST } : undefined;
    if (clip) {
      say(`${this.name}: cut at ${TALLEST}px of ${height}px`);
      await this.page.setViewportSize({ width: this.width, height: TALLEST });
    }
    const png = await this.page.screenshot({ fullPage: clip === undefined, clip, type: "png" });
    const picture = Picture.pack(png);
    say(`${this.name}: ${picture.width}x${picture.height}, ${picture.bytes.length} bytes in ${picture.colours} colours`);
    mkdirSync(plan.out, { recursive: true });
    writeFileSync(join(plan.out, `${this.name}.png`), picture.bytes);
    Picture.print(`${this.name}.png`, picture);
  }

  async close() {
    await this.context.close();
  }
}

/** The sessions of the plan's sign-ins, by name: each link is opened once in a browser of its own and its button pressed. */
const SESSIONS = new Map();

class Signin {
  static async all(browser, logins) {
    for (const login of logins) {
      const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
      try {
        const page = await context.newPage();
        const opened = await page.goto(login.url, { waitUntil: "load" });
        if (opened === null || opened.status() !== 200) throw new Error(`sign-in ${login.name} answered ${opened?.status()}`);
        await Promise.all([page.waitForURL((url) => !url.pathname.startsWith("/login/")), page.click("button[type=submit]")]);
        SESSIONS.set(login.name, await context.storageState());
        say(`signed in as ${login.name}, now at ${new URL(page.url()).pathname}`);
      } finally {
        await context.close();
      }
    }
  }
}

const folder = plan.serve ? await Folder.start(plan.serve) : null;
const browser = await Engines.start();
let failed = 0;
if (plan.probe) {
  const page = await browser.newPage();
  await page.setContent("<p style='font:20px Gaegu,serif'>probe</p>");
  say(`probe drew a page, chromium ${browser.version()}`);
} else {
  await Signin.all(browser, plan.logins ?? []);
  for (const shot of plan.shots) {
    for (const width of plan.widths) {
      const page = new Page(browser, shot, width);
      try {
        await page.open();
        await page.measure();
        await page.take();
      } catch (error) {
        failed += 1;
        say(`${page.name} FAILED: ${error.stack ?? error}`);
      } finally {
        await page.close().catch((error) => say(`${page.name} close: ${error.message}`));
      }
    }
  }
}
await browser.close();
folder?.close();
if (failed > 0) {
  say(`${failed} shots failed`);
  process.exit(1);
}
