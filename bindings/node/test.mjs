// Cargo produces a `wikitext_node.dll` (or `.so` / `.dylib`) and Node loads
// addons only with a `.node` extension, so the two names are bridged here.

import { createRequire } from "node:module";
import { copyFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");

function libraryName() {
  if (process.platform === "win32") return "wikitext_node.dll";
  if (process.platform === "darwin") return "libwikitext_node.dylib";
  return "libwikitext_node.so";
}

const built = join(root, "target", "debug", libraryName());
const addon = join(here, "wikitext_node.node");
if (!existsSync(built)) {
  console.error(`build the addon first: cargo build -p wikitext-node`);
  console.error(`expected ${built}`);
  process.exit(1);
}
copyFileSync(built, addon);

const require = createRequire(import.meta.url);
const wikitext = require(addon);

let failures = 0;
function check(name, body) {
  try {
    body();
    console.log(`${name.padEnd(48)} ok`);
  } catch (error) {
    console.log(`${name.padEnd(48)} FAIL: ${error.message}`);
    failures += 1;
  }
}

function equal(actual, expected, what) {
  if (actual !== expected) {
    throw new Error(`${what}: got ${JSON.stringify(actual)}, want ${JSON.stringify(expected)}`);
  }
}

check("version is reported", () => {
  if (typeof wikitext.version() !== "string" || wikitext.version() === "") {
    throw new Error("version() returned nothing");
  }
});

check("format codes are exposed", () => {
  const formats = wikitext.formats();
  equal(formats.html, 0, "html");
  equal(formats.markdown, 1, "markdown");
  equal(formats.wikitext, 2, "wikitext");
});

check("compiles a fragment", () => {
  equal(wikitext.wikitextToHtml("Hello '''world'''"), "<p>Hello <b>world</b></p>\n", "html");
});

check("compiles a document", () => {
  const html = wikitext.wikitextToDocument("x", "Title");
  if (!html.startsWith("<!DOCTYPE html>")) throw new Error(html.slice(0, 40));
});

check("templates from the options apply", () => {
  const options = new wikitext.WikitextOptions();
  options.title("Ada Lovelace");
  options.addTemplate("greet", "Hello, {{{1|world}}}!");
  equal(options.templateCount, 1, "templateCount");
  equal(wikitext.compile("{{greet|Rust}}", options), "<p>Hello, Rust!</p>\n", "html");
});

check("Lua modules from the options run", () => {
  const options = new wikitext.WikitextOptions();
  options.addModule(
    "Roman",
    "local p = {}\nfunction p.convert(frame) return 'MCMXCIV' end\nreturn p\n",
  );
  equal(options.moduleCount, 1, "moduleCount");
  equal(wikitext.compile("{{#invoke:Roman|convert|1994}}", options), "<p>MCMXCIV</p>\n", "html");
});

check("output mode is selectable", () => {
  const options = new wikitext.WikitextOptions();
  options.outputMode(1);
  const html = wikitext.compile("x", options);
  if (!html.includes("mw-parser-output")) throw new Error(html);
});

check("markdown becomes wikitext", () => {
  equal(
    wikitext.convert("# Title\n\nSome **bold** text.", 1, 2),
    "== Title ==\n\nSome '''bold''' text.",
    "wikitext",
  );
});

check("wikitext becomes markdown", () => {
  equal(
    wikitext.convert("== Title ==\n\nSome '''bold''' text.", 2, 1),
    "## Title\n\nSome **bold** text.",
    "markdown",
  );
});

check("wikitext becomes html through convert", () => {
  equal(wikitext.convert("'''x'''", 2, 0), "<p><b>x</b></p>\n", "html");
});

check("an unknown format is rejected", () => {
  let threw = false;
  try {
    wikitext.convert("x", 99, 2);
  } catch (error) {
    threw = true;
    if (!String(error.message).includes("format")) throw new Error(error.message);
  }
  if (!threw) throw new Error("an unknown format should throw");
});

check("describe reports the page", () => {
  const page = wikitext.describe("== One ==\n\nText.\n\n[[Category:Demo|Sort]]");
  equal(page.headings.length, 1, "headings");
  equal(page.headings[0].title, "One", "heading title");
  equal(page.headings[0].level, 2, "heading level");
  equal(page.categories.length, 1, "categories");
  equal(page.categories[0].name, "Demo", "category name");
  equal(page.categories[0].sortKey, "Sort", "sort key");
  if (!page.plainText.includes("One")) throw new Error(page.plainText);
  if (!Array.isArray(page.diagnostics)) throw new Error("diagnostics should be an array");
});

check("diagnostics are reported with a position", () => {
  const page = wikitext.describe("{| ");
  if (page.diagnostics.length === 0) throw new Error("expected a diagnostic");
  const first = page.diagnostics[0];
  if (typeof first.line !== "number") throw new Error(JSON.stringify(first));
  if (first.severity !== "warning") throw new Error(first.severity);
});

check("expand runs without parsing", () => {
  equal(wikitext.expand("{{#expr:6*7}}"), "42", "expanded");
});

check("entities decode", () => {
  equal(wikitext.decodeEntities("&amp;&lt;&#65;"), "&<A", "decoded");
});

check("the same options handle can be reused", () => {
  const options = new wikitext.WikitextOptions();
  options.addTemplate("t", "one");
  equal(wikitext.compile("{{t}}", options), "<p>one</p>\n", "first");
  options.addTemplate("t", "two");
  equal(wikitext.compile("{{t}}", options), "<p>two</p>\n", "second");
});

check("operations return values, not promises", () => {
  const result = wikitext.wikitextToHtml("x");
  if (typeof result !== "string") throw new Error(`got a ${typeof result}`);
  if (typeof result.then === "function") throw new Error("returned a thenable");
});

console.log(failures === 0 ? "\nALL NODE TESTS PASSED" : `\n${failures} NODE TESTS FAILED`);
process.exit(failures === 0 ? 0 : 1);
