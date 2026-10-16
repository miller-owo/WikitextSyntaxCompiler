#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include "wikitext.h"

static int failures = 0;
static void check(int ok, const char *what) {
    printf("%-46s %s\n", what, ok ? "ok" : "FAIL");
    if (!ok) failures++;
}

static char *text(const WikitextOwnedBuffer *b) {
    if (!b || !b->data) return strdup("");
    char *out = malloc(b->len + 1);
    memcpy(out, b->data, b->len);
    out[b->len] = 0;
    return out;
}

static int32_t host_resolve(void *ctx, const uint8_t *name, size_t name_len,
                            WikitextTemplateWriter body) {
    (void)ctx;
    if (name_len == 4 && memcmp(name, "host", 4) == 0) {
        const char *text = "from the host";
        return body(NULL, (const uint8_t *)text, strlen(text));
    }
    return WIKITEXT_ERROR_NULL;
}

int main(void) {
    WikitextOwnedBuffer buf;
    WikitextOptions *options = NULL;
    int32_t status;

    memset(&buf, 0, sizeof buf);
    check(wikitext_version(&buf) == WIKITEXT_OK, "version");
    char *v = text(&buf);
    printf("library version: %s\n", v);
    free(v);
    wikitext_buffer_free(&buf);
    check(buf.data == NULL && buf.len == 0, "free clears the header");

    check(wikitext_options_new(&options) == WIKITEXT_OK, "options_new");
    check(options != NULL, "options handle is not NULL");

    memset(&buf, 0, sizeof buf);
    const char *src = "Hello '''world'''";
    status = wikitext_compile((const uint8_t *)src, strlen(src), options, &buf);
    check(status == WIKITEXT_OK, "compile returns ok");
    char *html = text(&buf);
    check(strcmp(html, "<p>Hello <b>world</b></p>\n") == 0, "compile output matches");
    free(html);
    wikitext_buffer_free(&buf);

    const char *tname = "greet";
    const char *tbody = "Hello, {{{1|world}}}!";
    check(wikitext_options_add_template(options,
            (const uint8_t *)tname, strlen(tname),
            (const uint8_t *)tbody, strlen(tbody)) == WIKITEXT_OK, "add_template");
    check(wikitext_options_template_count(options) == 1, "template_count");

    memset(&buf, 0, sizeof buf);
    const char *call = "{{greet|Rust}}";
    check(wikitext_compile((const uint8_t *)call, strlen(call), options, &buf) == WIKITEXT_OK,
          "compile with template");
    html = text(&buf);
    check(strcmp(html, "<p>Hello, Rust!</p>\n") == 0, "template expanded");
    free(html);
    wikitext_buffer_free(&buf);

    const char *mname = "Roman";
    const char *msrc = "local p = {}\nfunction p.convert(f) return 'MCMXCIV' end\nreturn p\n";
    check(wikitext_options_add_module(options,
            (const uint8_t *)mname, strlen(mname),
            (const uint8_t *)msrc, strlen(msrc)) == WIKITEXT_OK, "add_module");
    check(wikitext_options_module_count(options) == 1, "module_count");

    memset(&buf, 0, sizeof buf);
    const char *invoke = "{{#invoke:Roman|convert|1994}}";
    check(wikitext_compile((const uint8_t *)invoke, strlen(invoke), options, &buf) == WIKITEXT_OK,
          "compile with #invoke");
    html = text(&buf);
    check(strcmp(html, "<p>MCMXCIV</p>\n") == 0, "#invoke ran Lua");
    free(html);
    wikitext_buffer_free(&buf);

    check(wikitext_options_set_resolver(options, host_resolve, NULL) == WIKITEXT_OK,
          "set_resolver");
    memset(&buf, 0, sizeof buf);
    const char *hostcall = "{{host}}";
    check(wikitext_compile((const uint8_t *)hostcall, strlen(hostcall), options, &buf) == WIKITEXT_OK,
          "compile with a host resolver");
    html = text(&buf);
    check(strcmp(html, "<p>from the host</p>\n") == 0, "host resolver was consulted");
    free(html);
    wikitext_buffer_free(&buf);
    wikitext_options_set_resolver(options, NULL, NULL);

    WikitextDocument *doc = NULL;
    const char *page = "== One ==\n\ntext\n\n== Two ==\n\n[[Category:Demo|Sort]]\n";
    check(wikitext_parse((const uint8_t *)page, strlen(page), options, &doc) == WIKITEXT_OK,
          "parse returns ok");
    check(doc != NULL, "document handle is not NULL");

    WikitextMetadata meta;
    memset(&meta, 0, sizeof meta);
    check(wikitext_document_metadata(doc, &meta) == WIKITEXT_OK, "document_metadata");
    check(meta.heading_count == 2, "two headings");
    check(meta.category_count == 1, "one category");
    if (meta.plain_text) {
        char *plain = text(meta.plain_text);
        check(strstr(plain, "One") != NULL, "plain text contains the heading");
        free(plain);
        wikitext_buffer_free(meta.plain_text);
    }

    WikitextHeading heading;
    memset(&heading, 0, sizeof heading);
    check(wikitext_document_heading(doc, 1, &heading) == WIKITEXT_OK, "heading index 1");
    if (heading.title) {
        char *title = text(heading.title);
        check(strcmp(title, "Two") == 0, "second heading title");
        free(title);
        wikitext_buffer_free(heading.title);
    }
    if (heading.id) wikitext_buffer_free(heading.id);

    WikitextCategory category;
    memset(&category, 0, sizeof category);
    check(wikitext_document_category(doc, 0, &category) == WIKITEXT_OK, "category index 0");
    if (category.name) {
        char *name = text(category.name);
        check(strcmp(name, "Demo") == 0, "category name");
        free(name);
        wikitext_buffer_free(category.name);
    }
    if (category.sort_key) {
        char *key = text(category.sort_key);
        check(strcmp(key, "Sort") == 0, "category sort key");
        free(key);
        wikitext_buffer_free(category.sort_key);
    }

    check(wikitext_document_heading(doc, 99, &heading) == WIKITEXT_ERROR_ARGUMENT,
          "out-of-range heading is reported");

    memset(&buf, 0, sizeof buf);
    check(wikitext_last_error(&buf) == WIKITEXT_OK, "last_error returns ok");
    char *message = text(&buf);
    check(strlen(message) > 0, "last_error carries a message");
    free(message);
    wikitext_buffer_free(&buf);

    wikitext_document_free(doc);
    wikitext_options_free(options);

    memset(&buf, 0, sizeof buf);
    const char *md = "# Title\n\nSome **bold** text.";
    check(wikitext_convert((const uint8_t *)md, strlen(md),
                           WIKITEXT_FORMAT_MARKDOWN, WIKITEXT_FORMAT_WIKITEXT,
                           NULL, &buf) == WIKITEXT_OK, "markdown -> wikitext");
    html = text(&buf);
    check(strcmp(html, "== Title ==\n\nSome '''bold''' text.") == 0, "markdown converted");
    free(html);
    wikitext_buffer_free(&buf);

    memset(&buf, 0, sizeof buf);
    const char *wt = "== Title ==\n\nSome '''bold''' text.";
    check(wikitext_convert((const uint8_t *)wt, strlen(wt),
                           WIKITEXT_FORMAT_WIKITEXT, WIKITEXT_FORMAT_MARKDOWN,
                           NULL, &buf) == WIKITEXT_OK, "wikitext -> markdown");
    html = text(&buf);
    check(strcmp(html, "## Title\n\nSome **bold** text.") == 0, "wikitext converted");
    free(html);
    wikitext_buffer_free(&buf);

    memset(&buf, 0, sizeof buf);
    const char *tags = "<h2>Two</h2><p>Text</p>";
    check(wikitext_convert((const uint8_t *)tags, strlen(tags),
                           WIKITEXT_FORMAT_HTML, WIKITEXT_FORMAT_WIKITEXT,
                           NULL, &buf) == WIKITEXT_OK, "html -> wikitext");
    html = text(&buf);
    check(strcmp(html, "== Two ==\n\nText") == 0, "html converted");
    free(html);
    wikitext_buffer_free(&buf);

    memset(&buf, 0, sizeof buf);
    const char *bold = "'''x'''";
    check(wikitext_convert((const uint8_t *)bold, strlen(bold),
                           WIKITEXT_FORMAT_WIKITEXT, WIKITEXT_FORMAT_HTML,
                           NULL, &buf) == WIKITEXT_OK, "wikitext -> html");
    html = text(&buf);
    check(strcmp(html, "<p><b>x</b></p>\n") == 0, "wikitext compiled");
    free(html);
    wikitext_buffer_free(&buf);

    check(wikitext_convert((const uint8_t *)"x", 1, 1, 99, NULL, &buf)
              == WIKITEXT_ERROR_ARGUMENT, "an unknown target format is rejected");

    printf("\n%s\n", failures == 0 ? "ALL C TESTS PASSED" : "SOME C TESTS FAILED");
    return failures == 0 ? 0 : 1;
}
