#!/usr/bin/env node
// Semantic-structure, CSS-off readability, and axe-core checks for one HTML file.
//
// This is the "does the page still make sense with the stylesheet ripped out?"
// half of the validation pipeline. W3C well-formedness lives in html5validator
// (see scripts/validate-html.sh); this script encodes the *semantic contract*
// of the wizard theme, which no off-the-shelf linter knows about.
//
// It parses the STATIC DOM with jsdom (no browser on purpose): a real browser
// would apply CSS/JS before we look, which is exactly the state we want to
// audit against. axe-core runs against that same static DOM.
//
// Then it runs the page's own scripts in jsdom twice (no stylesheet is ever
// loaded; "CSS on" is simulated via the --css-loaded sentinel that
// cssActive() reads — see docs/THEME.md):
//   - JS on, CSS off: the CSS-off rendering must not change. JS-only widgets
//     have to stay hidden (rule js-css-off-changed).
//   - JS on, CSS on: the markup the widgets build is held to the same
//     structural + axe rules as the static markup (rules prefixed "js:").
//
// Usage:   node scripts/validate-html.mjs <file.html> [<file.html> ...]
// Output:  human-readable findings, grouped ERROR / WARN, per file.
// Exit:    non-zero if any ERROR-level finding is present (WARN never fails).
//
// Deps (scripts/package.json, installed via `npm ci` by the orchestrator):
//   jsdom, axe-core
//
// Env:
//   VALIDATE_STRICT=1   promote WARN-level findings to ERROR (fail on them too)

import { readFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { dirname, join, relative, resolve, sep } from "node:path";
import { JSDOM, ResourceLoader, VirtualConsole } from "jsdom";
import axe from "axe-core";

const STRICT = process.env.VALIDATE_STRICT === "1";

// Built site root: pages under it are served to the JS passes from a fake
// origin so their root-relative /js/... URLs resolve. Files elsewhere (the
// fixtures) use their own directory as root.
const SITE_ROOT = resolve(process.env.VALIDATE_SITE_ROOT || "public");
const ORIGIN = "http://validate.invalid";

// How long the JS passes wait for a page's scripts to settle.
const JS_SETTLE_MS = 5000;

// Elements that render inline and carry text — the building blocks of the
// "word word word" run-together problem (see rule inline-run-together).
// <button> is left out: the UA draws it as a box even with CSS off.
const INLINE_TAGS = new Set([
  "A", "ABBR", "B", "BDI", "CITE", "CODE", "DATA", "DFN", "EM", "I",
  "KBD", "LABEL", "MARK", "Q", "S", "SAMP", "SMALL", "SPAN", "STRONG", "SUB",
  "SUP", "TIME", "U", "VAR",
]);

// Text that reads as a separator between inline items (footer "·" etc.).
const SEPARATOR_RE = /^[\s·•|,/–—:;-]+$/;

// axe rules that need real layout/CSS (contrast, visibility, orientation) are
// meaningless against a static jsdom DOM and irrelevant to "readable without
// CSS" — skip them so we don't emit noise or false negatives.
const AXE_SKIP_RULES = new Set([
  "color-contrast",
  "color-contrast-enhanced",
  "css-orientation-lock",
  "meta-viewport",
  "target-size",
  "scrollable-region-focusable",
]);

const ERROR = "ERROR";
const WARN = "WARN";

// ---------------------------------------------------------------------------
// Structural + CSS-off rules over a parsed Document.
// Each pushes { level, rule, msg } onto findings.
// ---------------------------------------------------------------------------
// `mode` is "static" for the shipped markup, "js" for the DOM after scripts
// ran; a few rules only make sense against the static markup.
function structuralFindings(doc, mode = "static") {
  const findings = [];
  const add = (level, rule, msg) => findings.push({ level, rule, msg });
  const $$ = (sel) => [...doc.querySelectorAll(sel)];

  // Hugo emits meta-refresh redirect stubs for language aliases (e.g.
  // /en/index.html -> /). They have no body content by design; auditing them
  // for landmarks/headings is a false positive, so skip.
  if (doc.querySelector('meta[http-equiv="refresh" i]') && !doc.querySelector("main, h1")) {
    return findings;
  }

  // --- Landmarks: exactly one <main>, at least one <nav> and a site <footer>.
  const mains = $$("main");
  if (mains.length === 0) add(ERROR, "landmark-main", "no <main> landmark");
  else if (mains.length > 1)
    add(ERROR, "landmark-main", `${mains.length} <main> landmarks (expected 1)`);

  if ($$("nav").length === 0)
    add(ERROR, "landmark-nav", "no <nav> landmark");

  if ($$("body > footer, footer.site-footer").length === 0)
    add(WARN, "landmark-footer", "no top-level <footer> landmark");

  // Advisory: is the site navigation wrapped in a <header> banner landmark?
  // A bare <nav> child of <body> has no banner role.
  const bareNav = $$("body > nav").length > 0;
  const hasBanner = $$("body > header, header[role=banner]").length > 0;
  if (bareNav && !hasBanner)
    add(WARN, "landmark-banner", "site <nav> is not wrapped in a <header> banner landmark");

  // --- Headings: exactly one <h1>, and no skipped levels (h1->h3 is a skip).
  const headings = $$("h1,h2,h3,h4,h5,h6");
  const h1s = headings.filter((h) => h.tagName === "H1");
  if (h1s.length === 0) add(ERROR, "heading-h1", "no <h1> on the page");
  else if (h1s.length > 1)
    add(ERROR, "heading-h1", `${h1s.length} <h1> elements (expected 1)`);

  let prev = 0;
  for (const h of headings) {
    const level = Number(h.tagName[1]);
    if (prev !== 0 && level > prev + 1) {
      add(
        ERROR,
        "heading-skip",
        `heading level jumps h${prev} -> h${level} (${describe(h)})`,
      );
    }
    prev = level;
  }

  // --- CSS-off: every linked <a href> must expose real text without CSS.
  // aria-label + an aria-hidden icon does NOT count: with CSS (icon font) off
  // the link renders empty. This is the nav-social-icons defect.
  for (const a of $$("a[href]")) {
    if (accessibleTextWithoutCss(a).length === 0) {
      add(
        ERROR,
        "link-empty-without-css",
        `link has no visible text when CSS is off (${describe(a)} -> ${a.getAttribute("href")})`,
      );
    }
  }

  // --- Placeholder links: href="#" / javascript: are buttons in disguise —
  // they go nowhere without JS. Use <button type="button">.
  for (const a of $$('a[href="#"], a[href^="javascript:" i]')) {
    add(ERROR, "link-placeholder", `link with placeholder href "${a.getAttribute("href")}" — use a <button> (${describe(a)})`);
  }

  // --- CSS-off: every <button> needs real text too. Same reasoning as links:
  // an aria-label on an icon-only button renders as an empty box with CSS
  // off. Put the label in a .visually-hidden span instead.
  for (const b of $$("button")) {
    if (accessibleTextWithoutCss(b).length === 0)
      add(ERROR, "button-empty-without-css", `button has no visible text when CSS is off (${describe(b)})`);
  }

  // --- JS-only controls must ship hidden. A <button> outside a form does
  // nothing without JS (unless it uses declarative popover/command wiring),
  // so it must sit in a [hidden] subtree that its script reveals.
  if (mode === "static") {
    for (const b of $$("button")) {
      if (b.closest("form") || b.hasAttribute("popovertarget") || b.hasAttribute("commandfor")) continue;
      if (b.closest("[hidden]")) continue;
      add(ERROR, "button-dead-without-js", `button outside a form is visible without JS — ship it (or its widget) with the hidden attribute (${describe(b)})`);
    }
  }

  // --- aria-label on an element with visible text replaces that text for
  // assistive tech (the old "Read more →" + aria-label pattern). Add context
  // with a .visually-hidden span instead, which also shows with CSS off.
  for (const el of $$("a[href][aria-label], button[aria-label]")) {
    if (accessibleTextWithoutCss(el).length > 0)
      add(WARN, "aria-label-overrides-text", `aria-label overrides visible text; use a .visually-hidden span (${describe(el)})`);
  }

  // --- Nested <nav>: a landmark inside a landmark of the same kind. Sub-groups
  // of a navigation are lists, not more <nav>s.
  for (const n of $$("nav nav")) {
    add(WARN, "landmark-nav-nested", `<nav> nested inside another <nav> — use a list (${describe(n)})`);
  }

  // --- Run-together inline items: three or more inline elements side by side
  // with nothing but whitespace between them read as one blob with CSS off
  // ("GitHub LinkedIn Mastodon Email"). Make them a list.
  for (const el of $$("body *")) {
    if (el.closest("[hidden], pre, code, script, style, template, svg")) continue;
    if (INLINE_TAGS.has(el.tagName)) continue; // judged by its parent
    const items = [...el.children];
    if (items.length < 3 || !items.every((c) => INLINE_TAGS.has(c.tagName))) continue;
    const withText = items.filter((c) => accessibleTextWithoutCss(c).length > 0);
    if (withText.length < 3) continue;
    const separated =
      [...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim() !== "") ||
      items.some((c) => SEPARATOR_RE.test(c.textContent) && c.textContent.trim() !== "");
    if (!separated)
      add(WARN, "inline-run-together", `${withText.length} inline items with no separator run together without CSS — use a list (${describe(el)})`);
  }

  // --- Empty decorative elements: a <div>/<span> with no content exists only
  // to be styled (backgrounds, overlays, dividers). Use ::before/::after.
  // Elements with an id are exempt: those are JS mount points. In JS-built
  // widgets (which only exist with CSS on) aria-hidden decoration is fine,
  // e.g. the memory card faces; static markup gets no such pass.
  for (const el of $$("div, span")) {
    if (el.id || el.children.length > 0 || el.textContent.trim() !== "") continue;
    if (el.closest("pre, code, svg")) continue;
    if (mode === "js" && el.closest('[aria-hidden="true"]')) continue;
    add(WARN, "empty-element", `empty <${el.tagName.toLowerCase()}> used for decoration — use a pseudo-element (${describe(el)})`);
  }

  // --- Images: need alt. Empty alt="" is only OK for decorative images that
  // are also hidden from AT.
  for (const img of $$("img")) {
    if (!img.hasAttribute("alt")) {
      add(ERROR, "img-alt-missing", `<img> without alt attribute (${img.getAttribute("src") || "?"})`);
    } else if (img.getAttribute("alt").trim() === "" && img.getAttribute("aria-hidden") !== "true") {
      add(
        WARN,
        "img-alt-empty",
        `<img> has empty alt but is not aria-hidden (decorative?) (${img.getAttribute("src") || "?"})`,
      );
    }
  }

  // --- Empty landmarks read as dead weight without CSS.
  for (const el of $$("main, nav, header, footer, section, article")) {
    if (visibleText(el).length === 0 && el.querySelectorAll("img[alt]:not([alt='']), a[href]").length === 0)
      add(WARN, "landmark-empty", `<${el.tagName.toLowerCase()}> landmark has no text content (${describe(el)})`);
  }

  return findings;
}

// Accessible text as a *sighted* user sees it with CSS disabled: text nodes of
// the element and its descendants, EXCLUDING aria-hidden subtrees (icon fonts,
// which vanish without CSS). aria-label is deliberately NOT counted here.
function accessibleTextWithoutCss(el) {
  return [...el.childNodes]
    .map((n) => {
      if (n.nodeType === 3) return n.textContent; // text node
      if (n.nodeType !== 1) return "";
      if (n.getAttribute && n.getAttribute("aria-hidden") === "true") return "";
      return accessibleTextWithoutCss(n);
    })
    .join("")
    .replace(/\s+/g, " ")
    .trim();
}

function visibleText(el) {
  return (el.textContent || "").replace(/\s+/g, " ").trim();
}

function describe(el) {
  const tag = el.tagName.toLowerCase();
  const id = el.id ? `#${el.id}` : "";
  const cls = el.getAttribute("class");
  const c = cls ? `.${cls.trim().split(/\s+/).join(".")}` : "";
  return `${tag}${id}${c}`;
}

// ---------------------------------------------------------------------------
// axe-core over the same static DOM.
// ---------------------------------------------------------------------------
async function axeFindings(dom) {
  const { window } = dom;
  window.eval(axe.source);
  const results = await window.axe.run(window.document, {
    resultTypes: ["violations"],
    rules: Object.fromEntries([...AXE_SKIP_RULES].map((r) => [r, { enabled: false }])),
  });
  return results.violations.map((v) => ({
    level: v.impact === "critical" || v.impact === "serious" ? ERROR : WARN,
    rule: `axe:${v.id}`,
    msg: `${v.help} (${v.nodes.length} node${v.nodes.length === 1 ? "" : "s"}) ${v.helpUrl}`,
  }));
}

// ---------------------------------------------------------------------------
// JS passes: run the page's own scripts in jsdom.
// ---------------------------------------------------------------------------

// Serves same-origin scripts from the site root; everything else (styles,
// fonts, images, other origins) is skipped. No stylesheet is ever applied.
class SiteLoader extends ResourceLoader {
  constructor(root) {
    super();
    this.root = root;
  }
  fetch(url, options) {
    const u = new URL(url);
    if (u.origin !== ORIGIN || !/\.m?js$/.test(u.pathname)) return null;
    // `hugo server` injects its dev-only live reload client into public/.
    if (u.pathname === "/livereload.js") return null;
    const path = join(this.root, decodeURIComponent(u.pathname));
    if (!path.startsWith(this.root + sep)) return null;
    return readFile(path);
  }
}

// Where `file` lives on the fake origin, and which root serves its assets.
function pageLocation(file) {
  const abs = resolve(file);
  const root = abs.startsWith(SITE_ROOT + sep) ? SITE_ROOT : dirname(abs);
  const path = relative(root, abs).split(sep).join("/");
  return { root, url: `${ORIGIN}/${path}` };
}

// Stand-ins for browser APIs jsdom lacks, plus the "is CSS on?" switch. The
// stubs are inert on purpose: no network, no media query ever matches.
function installStubs(window, cssOn) {
  const mql = (media) => ({
    media, matches: false, onchange: null,
    addEventListener() {}, removeEventListener() {},
    addListener() {}, removeListener() {}, dispatchEvent() { return false; },
  });
  window.matchMedia = mql;
  window.fetch = () => new Promise(() => {}); // never settles
  window.WebSocket = class {
    constructor() { this.readyState = 0; }
    send() {}
    close() {}
  };
  // cssActive() reads the --css-loaded sentinel the real stylesheet sets.
  const getPropertyValue = window.CSSStyleDeclaration.prototype.getPropertyValue;
  window.CSSStyleDeclaration.prototype.getPropertyValue = function (name) {
    if (name === "--css-loaded") return cssOn ? "1" : "";
    return getPropertyValue.call(this, name);
  };
}

async function runWithScripts(html, file, cssOn) {
  const { root, url } = pageLocation(file);
  const errors = [];
  const virtualConsole = new VirtualConsole();
  virtualConsole.on("jsdomError", (e) => errors.push(e.message.split("\n")[0]));
  const dom = new JSDOM(html, {
    url,
    runScripts: "dangerously",
    resources: new SiteLoader(root),
    pretendToBeVisual: true,
    virtualConsole,
    beforeParse: (window) => installStubs(window, cssOn),
  });
  await new Promise((done) => {
    const timer = setTimeout(done, JS_SETTLE_MS);
    dom.window.addEventListener("load", () => {
      clearTimeout(timer);
      setTimeout(done, 0); // let load handlers finish
    });
  });
  return { dom, errors };
}

// What a sighted reader gets with CSS off: text outside [hidden] subtrees,
// plus which controls are visible. Used to prove JS left that untouched.
function cssOffRendering(doc) {
  const parts = [];
  const walk = (node) => {
    for (const n of node.childNodes) {
      if (n.nodeType === 3) parts.push(n.textContent);
      if (n.nodeType !== 1) continue;
      if (n.hasAttribute("hidden") || /^(SCRIPT|STYLE|TEMPLATE|NOSCRIPT)$/.test(n.tagName)) continue;
      if (/^(A|BUTTON|INPUT|SELECT|TEXTAREA)$/.test(n.tagName)) parts.push(` [${n.tagName.toLowerCase()}] `);
      walk(n);
    }
  };
  walk(doc.body);
  return parts.join("").replace(/\s+/g, " ").trim();
}

// First differing stretch of two strings, for a readable error message.
function firstDifference(a, b) {
  let i = 0;
  while (i < a.length && a[i] === b[i]) i++;
  const from = Math.max(0, i - 30);
  return `static "…${a.slice(from, i + 40)}…" vs after JS "…${b.slice(from, i + 40)}…"`;
}

function scriptErrorFindings(errors, pass) {
  return [...new Set(errors)].map((msg) => ({ level: WARN, rule: "js-error", msg: `${pass}: ${msg}` }));
}

// ---------------------------------------------------------------------------
async function checkFile(file) {
  const html = readFileSync(file, "utf8");
  // runScripts:"outside-only" gives us a working window.eval (needed to inject
  // axe.source); pretendToBeVisual satisfies axe's requestAnimationFrame use.
  // resources are NOT loaded — we audit the static markup, nothing fetched.
  const dom = new JSDOM(html, {
    url: pageLocation(file).url,
    runScripts: "outside-only",
    pretendToBeVisual: true,
  });
  const findings = structuralFindings(dom.window.document);
  findings.push(...(await axeFindings(dom)));
  const staticRendering = cssOffRendering(dom.window.document);
  dom.window.close();

  // Redirect stubs have nothing to run.
  if (/<meta[^>]+http-equiv=["']?refresh/i.test(html) && !/<main[\s>]/i.test(html)) return findings;

  // JS on, CSS off: the CSS-off rendering must be exactly the static one.
  {
    const { dom: jsDom, errors } = await runWithScripts(html, file, false);
    const after = cssOffRendering(jsDom.window.document);
    if (after !== staticRendering)
      findings.push({
        level: ERROR,
        rule: "js-css-off-changed",
        msg: `scripts changed the page with CSS off (JS-only UI must stay hidden unless cssActive()): ${firstDifference(staticRendering, after)}`,
      });
    findings.push(...scriptErrorFindings(errors, "JS on, CSS off"));
    jsDom.window.close();
  }

  // JS on, CSS on: audit the markup the widgets build. Only report what the
  // static pass didn't already, so each problem shows up once.
  {
    const { dom: jsDom, errors } = await runWithScripts(html, file, true);
    const seen = new Set(findings.map((f) => `${f.rule}|${f.msg}`));
    const jsFindings = [...structuralFindings(jsDom.window.document, "js"), ...(await axeFindings(jsDom))];
    for (const f of jsFindings) {
      if (!seen.has(`${f.rule}|${f.msg}`)) findings.push({ ...f, rule: `js:${f.rule}` });
    }
    findings.push(...scriptErrorFindings(errors, "JS on, CSS on"));
    jsDom.window.close();
  }

  return findings;
}

function report(file, findings) {
  const errors = findings.filter((f) => f.level === ERROR || (STRICT && f.level === WARN));
  const warns = findings.filter((f) => f.level === WARN && !STRICT);
  const status = errors.length ? "FAIL" : warns.length ? "warn" : "ok";
  console.log(`[${status}] ${file}`);
  for (const f of findings) {
    const lvl = f.level === ERROR || (STRICT && f.level === WARN) ? ERROR : WARN;
    console.log(`    ${lvl.padEnd(5)} ${f.rule}: ${f.msg}`);
  }
  return errors.length;
}

async function main() {
  const files = process.argv.slice(2);
  if (files.length === 0) {
    console.error("usage: validate-html.mjs <file.html> [<file.html> ...]");
    process.exit(2);
  }
  let errorCount = 0;
  for (const file of files) {
    try {
      errorCount += report(file, await checkFile(file));
    } catch (e) {
      console.log(`[FAIL] ${file}`);
      console.log(`    ERROR  parse: ${e.message}`);
      errorCount += 1;
    }
  }
  process.exit(errorCount ? 1 : 0);
}

main();
