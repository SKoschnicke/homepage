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
import { pathToFileURL } from "node:url";
import { JSDOM } from "jsdom";
import axe from "axe-core";

const STRICT = process.env.VALIDATE_STRICT === "1";

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
function structuralFindings(doc) {
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

  // --- CSS-off: every <button> likewise needs text OR an aria-label.
  //   (Buttons are usually JS-driven controls; an aria-label is acceptable
  //    since they're inert without CSS/JS anyway — WARN, not ERROR.)
  for (const b of $$("button")) {
    const hasText = visibleText(b).length > 0;
    const hasLabel = (b.getAttribute("aria-label") || "").trim().length > 0;
    if (!hasText && !hasLabel)
      add(WARN, "button-unlabeled", `button has no text and no aria-label (${describe(b)})`);
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
async function checkFile(file) {
  const html = readFileSync(file, "utf8");
  // runScripts:"outside-only" gives us a working window.eval (needed to inject
  // axe.source); pretendToBeVisual satisfies axe's requestAnimationFrame use.
  // resources are NOT loaded — we audit the static markup, nothing fetched.
  const dom = new JSDOM(html, {
    url: pathToFileURL(file).href,
    runScripts: "outside-only",
    pretendToBeVisual: true,
  });
  const findings = structuralFindings(dom.window.document);
  findings.push(...(await axeFindings(dom)));
  dom.window.close();
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
