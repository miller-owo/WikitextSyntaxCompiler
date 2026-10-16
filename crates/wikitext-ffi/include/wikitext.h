/*
 * No call aborts the process, and a handle is not thread safe.
 */

#ifndef WIKITEXT_H
#define WIKITEXT_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define WIKITEXT_OK 0
#define WIKITEXT_ERROR_NULL (-1)
#define WIKITEXT_ERROR_PANIC (-2)
#define WIKITEXT_ERROR_UTF8 (-3)
#define WIKITEXT_ERROR_ARGUMENT (-4)
#define WIKITEXT_ERROR_IO (-5)

#define WIKITEXT_OUTPUT_FRAGMENT 0
#define WIKITEXT_OUTPUT_PARSER_OUTPUT 1
#define WIKITEXT_OUTPUT_DOCUMENT 2

#define WIKITEXT_OPTION_PRETTY 0
#define WIKITEXT_OPTION_SHOW_CATEGORIES 1
#define WIKITEXT_OPTION_SHOW_IMAGES 2
#define WIKITEXT_OPTION_DIAGNOSTICS 3
#define WIKITEXT_OPTION_HEADING_ANCHORS 4
#define WIKITEXT_OPTION_EDIT_SECTION_LINKS 5
#define WIKITEXT_OPTION_KEEP_COMMENTS 6

#define WIKITEXT_INT_MAX_EXPANSION_DEPTH 0

#define WIKITEXT_FORMAT_HTML 0
#define WIKITEXT_FORMAT_MARKDOWN 1
#define WIKITEXT_FORMAT_WIKITEXT 2

#define WIKITEXT_SEVERITY_NOTE 0
#define WIKITEXT_SEVERITY_WARNING 1
#define WIKITEXT_SEVERITY_ERROR 2

/*
 * `data` is never NUL terminated and is NULL when `len` is 0. Free it with
 * wikitext_buffer_free. A struct the library filled in is always a value: free
 * its data, never the struct itself.
 */
typedef struct WikitextOwnedBuffer {
    uint8_t *data;
    size_t len;
    size_t capacity;
} WikitextOwnedBuffer;

typedef int64_t (*WikitextReaderCallback)(void *ctx, uint8_t *buf, size_t len);

typedef int64_t (*WikitextWriterCallback)(void *ctx, const uint8_t *buf, size_t len);

typedef int32_t (*WikitextTemplateWriter)(void *ctx, const uint8_t *body, size_t body_len);

typedef int32_t (*WikitextResolveCallback)(void *ctx,
                                          const uint8_t *name,
                                          size_t name_len,
                                          WikitextTemplateWriter body);

typedef struct WikitextReader {
    void *ctx;
    WikitextReaderCallback read;
} WikitextReader;

typedef struct WikitextWriter {
    void *ctx;
    WikitextWriterCallback write;
} WikitextWriter;

typedef struct WikitextOptions WikitextOptions;
typedef struct WikitextDocument WikitextDocument;

typedef struct WikitextMetadata {
    WikitextOwnedBuffer *title;
    WikitextOwnedBuffer *redirect;
    WikitextOwnedBuffer *display_title;
    WikitextOwnedBuffer *default_sort;
    WikitextOwnedBuffer *plain_text;
    size_t heading_count;
    size_t category_count;
    size_t diagnostic_count;
    size_t reference_count;
    size_t external_link_count;
    bool toc_none;
    bool toc_force;
    bool toc_inline;
    bool trailing_newline;
} WikitextMetadata;

typedef struct WikitextHeading {
    uint8_t level;
    WikitextOwnedBuffer *id;
    WikitextOwnedBuffer *title;
} WikitextHeading;

typedef struct WikitextCategory {
    WikitextOwnedBuffer *name;
    WikitextOwnedBuffer *sort_key;
} WikitextCategory;

typedef struct WikitextDiagnostic {
    int32_t severity;
    WikitextOwnedBuffer *code;
    WikitextOwnedBuffer *message;
    uint32_t line;
    uint32_t column;
} WikitextDiagnostic;

int32_t wikitext_version(WikitextOwnedBuffer *out);

void wikitext_free(void *ptr);
void wikitext_buffer_free(WikitextOwnedBuffer *buffer);

int32_t wikitext_last_error(WikitextOwnedBuffer *out);
void wikitext_clear_error(void);

int32_t wikitext_options_new(WikitextOptions **out);
void wikitext_options_free(WikitextOptions *options);

int32_t wikitext_options_set_title(WikitextOptions *options,
                                  const uint8_t *title,
                                  size_t title_len);
int32_t wikitext_options_set_language(WikitextOptions *options,
                                     const uint8_t *language,
                                     size_t language_len);
int32_t wikitext_options_set_article_path(WikitextOptions *options,
                                         const uint8_t *path,
                                         size_t path_len);
int32_t wikitext_options_set_server_url(WikitextOptions *options,
                                       const uint8_t *url,
                                       size_t url_len);
int32_t wikitext_options_set_image_base_url(WikitextOptions *options,
                                           const uint8_t *url,
                                           size_t url_len);
int32_t wikitext_options_set_output_mode(WikitextOptions *options, int32_t mode);
int32_t wikitext_options_set_bool(WikitextOptions *options, int32_t field, bool value);
int32_t wikitext_options_set_int(WikitextOptions *options, int32_t field, size_t value);

int32_t wikitext_options_set_resolver(WikitextOptions *options,
                                     WikitextResolveCallback callback,
                                     void *ctx);

int32_t wikitext_options_add_template(WikitextOptions *options,
                                     const uint8_t *name,
                                     size_t name_len,
                                     const uint8_t *body,
                                     size_t body_len);

int32_t wikitext_options_add_module(WikitextOptions *options,
                                   const uint8_t *name,
                                   size_t name_len,
                                   const uint8_t *source,
                                   size_t source_len);

size_t wikitext_options_template_count(const WikitextOptions *options);
size_t wikitext_options_module_count(const WikitextOptions *options);

int32_t wikitext_compile(const uint8_t *source,
                        size_t source_len,
                        const WikitextOptions *options,
                        WikitextOwnedBuffer *out);

int32_t wikitext_compile_reader(const WikitextReader *reader,
                               const WikitextOptions *options,
                               const WikitextWriter *writer);

int32_t wikitext_expand(const uint8_t *source,
                       size_t source_len,
                       const WikitextOptions *options,
                       WikitextOwnedBuffer *out);

int32_t wikitext_decode_entities(const uint8_t *source,
                                size_t source_len,
                                WikitextOwnedBuffer *out);

/*
 * Only `to` HTML consults the templates and modules on `options`.
 */
int32_t wikitext_convert(const uint8_t *source,
                         size_t source_len,
                         int32_t from,
                         int32_t to,
                         const WikitextOptions *options,
                         WikitextOwnedBuffer *out);

int32_t wikitext_parse(const uint8_t *source,
                      size_t source_len,
                      const WikitextOptions *options,
                      WikitextDocument **out);

void wikitext_document_free(WikitextDocument *document);

int32_t wikitext_document_html(const WikitextDocument *document,
                              WikitextOwnedBuffer *out);
int32_t wikitext_document_render_to(const WikitextDocument *document,
                                   const WikitextWriter *writer);

int32_t wikitext_document_metadata(const WikitextDocument *document,
                                  WikitextMetadata *out);

int32_t wikitext_document_heading(const WikitextDocument *document,
                                 size_t index,
                                 WikitextHeading *out);
int32_t wikitext_document_category(const WikitextDocument *document,
                                  size_t index,
                                  WikitextCategory *out);
int32_t wikitext_document_diagnostic(const WikitextDocument *document,
                                    size_t index,
                                    WikitextDiagnostic *out);

#ifdef __cplusplus
}
#endif

#endif /* WIKITEXT_H */
