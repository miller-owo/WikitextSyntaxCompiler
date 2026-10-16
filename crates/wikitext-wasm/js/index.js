// A thin wrapper so callers do not have to load the module by hand or remember
// the format codes.

export const FORMAT = {
  HTML: 0,
  MARKDOWN: 1,
  WIKITEXT: 2,
};

export const OUTPUT = {
  FRAGMENT: 0,
  PARSER_OUTPUT: 1,
  DOCUMENT: 2,
};

export async function load(module) {
  const bindings = module ?? (await import("./wikitext_wasm.js"));

  return {
    version: () => bindings.wikitextVersion(),

    Options: bindings.WikitextOptions,

    toHtml: (source) => bindings.wikitextToHtml(source),

    toDocument: (source, title) => bindings.wikitextToDocument(source, title),

    compile: (source, options) => bindings.wikitextCompile(source, options),

    convert: (source, from, to) => bindings.wikitextConvert(source, from, to),

    describe: (source, options) =>
      JSON.parse(bindings.wikitextDescribe(source, options ?? undefined)),

    diagnostics: (source) => JSON.parse(bindings.wikitextDiagnostics(source)),

    expand: (source, options) =>
      bindings.wikitextExpand(source, options ?? undefined),
  };
}

export default load;
