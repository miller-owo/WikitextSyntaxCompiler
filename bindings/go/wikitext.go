// Package wikitext compiles MediaWiki wikitext to HTML through the C ABI, so the
// shared library must be on the loader path at run time — see README.md.
//
// Options and Document are not safe for concurrent use: give each goroutine its
// own.
package wikitext

/*
#include <stdlib.h>
#include "wikitext.h"

// A Go function cannot be a C function pointer, so this is bridged through an
// exported Go function and dispatched on an integer key.
extern int32_t wikitextGoResolve(void *ctx, uint8_t *name, size_t name_len,
                                 WikitextTemplateWriter body);

static inline int32_t wikitextSetResolverGo(WikitextOptions *o, uintptr_t key) {
    return wikitext_options_set_resolver(
        o, (WikitextResolveCallback)(void *)wikitextGoResolve, (void *)key);
}

static inline int32_t wikitextCallTemplateBody(WikitextTemplateWriter body, void *ctx,
                                               const uint8_t *data, size_t len) {
    return body ? body(ctx, data, len) : WIKITEXT_ERROR_NULL;
}
*/
import "C"

import (
	"errors"
	"fmt"
	"sync"
	"unsafe"
)

var (
	ErrNull     = errors.New("wikitext: null pointer or handle")
	ErrPanic    = errors.New("wikitext: the library panicked")
	ErrUTF8     = errors.New("wikitext: input is not valid UTF-8")
	ErrArgument = errors.New("wikitext: invalid argument")
	ErrIO       = errors.New("wikitext: an io callback failed")
)

type Error struct {
	Status  int
	Base    error
	Message string
}

func (e *Error) Error() string { return "wikitext: " + e.Message }

func (e *Error) Unwrap() error { return e.Base }

var errorBases = map[C.int32_t]error{
	C.WIKITEXT_ERROR_NULL:     ErrNull,
	C.WIKITEXT_ERROR_PANIC:    ErrPanic,
	C.WIKITEXT_ERROR_UTF8:     ErrUTF8,
	C.WIKITEXT_ERROR_ARGUMENT: ErrArgument,
	C.WIKITEXT_ERROR_IO:       ErrIO,
}

func statusError(status C.int32_t) error {
	if status == C.WIKITEXT_OK {
		return nil
	}
	base, known := errorBases[status]
	if !known {
		base = errors.New("wikitext: unknown failure")
	}
	message := lastError()
	if message == "" {
		return base
	}
	return &Error{Status: int(status), Base: base, Message: message}
}

func lastError() string {
	var buffer C.WikitextOwnedBuffer
	if C.wikitext_last_error(&buffer) != C.WIKITEXT_OK {
		return ""
	}
	return takeBuffer(&buffer)
}

func takeBuffer(buffer *C.WikitextOwnedBuffer) string {
	if buffer == nil || buffer.data == nil || buffer.len == 0 {
		C.wikitext_buffer_free(buffer)
		return ""
	}
	text := C.GoStringN((*C.char)(unsafe.Pointer(buffer.data)), C.int(buffer.len))
	C.wikitext_buffer_free(buffer)
	return text
}

func Version() string {
	var buffer C.WikitextOwnedBuffer
	if C.wikitext_version(&buffer) != C.WIKITEXT_OK {
		return ""
	}
	return takeBuffer(&buffer)
}

type OutputMode int

const (
	Fragment     OutputMode = C.WIKITEXT_OUTPUT_FRAGMENT
	ParserOutput OutputMode = C.WIKITEXT_OUTPUT_PARSER_OUTPUT
	FullDocument OutputMode = C.WIKITEXT_OUTPUT_DOCUMENT
)

type BoolOption int

const (
	Pretty           BoolOption = C.WIKITEXT_OPTION_PRETTY
	ShowCategories   BoolOption = C.WIKITEXT_OPTION_SHOW_CATEGORIES
	ShowImages       BoolOption = C.WIKITEXT_OPTION_SHOW_IMAGES
	Diagnostics      BoolOption = C.WIKITEXT_OPTION_DIAGNOSTICS
	HeadingAnchors   BoolOption = C.WIKITEXT_OPTION_HEADING_ANCHORS
	EditSectionLinks BoolOption = C.WIKITEXT_OPTION_EDIT_SECTION_LINKS
	KeepComments     BoolOption = C.WIKITEXT_OPTION_KEEP_COMMENTS
)

type Options struct {
	handle   *C.WikitextOptions
	resolver Resolver
	key      uintptr
}

func NewOptions() (*Options, error) {
	var handle *C.WikitextOptions
	if status := C.wikitext_options_new(&handle); status != C.WIKITEXT_OK {
		return nil, statusError(status)
	}
	return &Options{handle: handle}, nil
}

func (o *Options) Close() {
	if o == nil || o.handle == nil {
		return
	}
	if o.key != 0 {
		resolvers.Delete(o.key)
		o.key = 0
		o.resolver = nil
	}
	C.wikitext_options_free(o.handle)
	o.handle = nil
}

type stringSetter func(*C.WikitextOptions, *C.uint8_t, C.size_t) C.int32_t

func (o *Options) setString(setter stringSetter, value string) error {
	if o == nil || o.handle == nil {
		return ErrNull
	}
	// An empty value is still a value, so it needs a pointer; a nil pointer with
	// a zero length would read as "absent" instead.
	if len(value) > 0 {
		bytes := []byte(value)
		pointer := (*C.uint8_t)(unsafe.Pointer(&bytes[0]))
		return statusError(setter(o.handle, pointer, C.size_t(len(value))))
	}
	placeholder := C.malloc(1)
	defer C.free(placeholder)
	return statusError(setter(o.handle, (*C.uint8_t)(placeholder), 0))
}

func (o *Options) SetTitle(title string) error {
	return o.setString(func(h *C.WikitextOptions, p *C.uint8_t, n C.size_t) C.int32_t {
		return C.wikitext_options_set_title(h, p, n)
	}, title)
}

func (o *Options) SetLanguage(language string) error {
	return o.setString(func(h *C.WikitextOptions, p *C.uint8_t, n C.size_t) C.int32_t {
		return C.wikitext_options_set_language(h, p, n)
	}, language)
}

func (o *Options) SetArticlePath(path string) error {
	return o.setString(func(h *C.WikitextOptions, p *C.uint8_t, n C.size_t) C.int32_t {
		return C.wikitext_options_set_article_path(h, p, n)
	}, path)
}

func (o *Options) SetServerURL(url string) error {
	return o.setString(func(h *C.WikitextOptions, p *C.uint8_t, n C.size_t) C.int32_t {
		return C.wikitext_options_set_server_url(h, p, n)
	}, url)
}

func (o *Options) SetImageBaseURL(url string) error {
	return o.setString(func(h *C.WikitextOptions, p *C.uint8_t, n C.size_t) C.int32_t {
		return C.wikitext_options_set_image_base_url(h, p, n)
	}, url)
}

func (o *Options) SetOutputMode(mode OutputMode) error {
	if o == nil || o.handle == nil {
		return ErrNull
	}
	return statusError(C.wikitext_options_set_output_mode(o.handle, C.int32_t(mode)))
}

func (o *Options) SetBool(field BoolOption, value bool) error {
	if o == nil || o.handle == nil {
		return ErrNull
	}
	return statusError(C.wikitext_options_set_bool(o.handle, C.int32_t(field), C.bool(value)))
}

func (o *Options) SetMaxExpansionDepth(depth int) error {
	if o == nil || o.handle == nil {
		return ErrNull
	}
	return statusError(C.wikitext_options_set_int(o.handle, 0, C.size_t(depth)))
}

func addEntry(options *C.WikitextOptions, name, text string, add func(*C.WikitextOptions, *C.uint8_t, C.size_t, *C.uint8_t, C.size_t) C.int32_t) error {
	if options == nil {
		return ErrNull
	}
	nameBytes := []byte(name)
	textBytes := []byte(text)
	return statusError(add(
		options,
		bytesPointer(nameBytes), C.size_t(len(nameBytes)),
		bytesPointer(textBytes), C.size_t(len(textBytes)),
	))
}

func (o *Options) AddTemplate(name, body string) error {
	if o == nil {
		return ErrNull
	}
	return addEntry(o.handle, name, body, func(h *C.WikitextOptions, n *C.uint8_t, nl C.size_t, b *C.uint8_t, bl C.size_t) C.int32_t {
		return C.wikitext_options_add_template(h, n, nl, b, bl)
	})
}

func (o *Options) AddModule(name, source string) error {
	if o == nil {
		return ErrNull
	}
	return addEntry(o.handle, name, source, func(h *C.WikitextOptions, n *C.uint8_t, nl C.size_t, s *C.uint8_t, sl C.size_t) C.int32_t {
		return C.wikitext_options_add_module(h, n, nl, s, sl)
	})
}

func (o *Options) TemplateCount() int {
	if o == nil || o.handle == nil {
		return 0
	}
	return int(C.wikitext_options_template_count(o.handle))
}

func (o *Options) ModuleCount() int {
	if o == nil || o.handle == nil {
		return 0
	}
	return int(C.wikitext_options_module_count(o.handle))
}

// cgo may not retain a pointer to Go memory once the call returns, so the slice
// has to stay alive across it — holding it in the call's own argument does that.
func bytesPointer(bytes []byte) *C.uint8_t {
	if len(bytes) == 0 {
		return nil
	}
	return (*C.uint8_t)(unsafe.Pointer(&bytes[0]))
}

type Resolver func(name string) (body string, found bool)

// The resolver runs on the goroutine that compiles and must not call back into
// the same Options. Passing nil removes it.
func (o *Options) SetResolver(resolver Resolver) error {
	if o == nil || o.handle == nil {
		return ErrNull
	}
	if resolver == nil {
		if o.key != 0 {
			resolvers.Delete(o.key)
			o.key = 0
		}
		o.resolver = nil
		return statusError(C.wikitextSetResolverGo(o.handle, 0))
	}
	if o.key == 0 {
		o.key = nextResolverKey()
	}
	o.resolver = resolver
	resolvers.Store(o.key, resolver)
	return statusError(C.wikitextSetResolverGo(o.handle, C.uintptr_t(o.key)))
}

// The library only ever sees the integer key, never a Go pointer, which is what
// keeps this inside cgo's pointer rules.
var (
	resolvers   sync.Map
	resolverKey uintptr
	resolverMu  sync.Mutex
)

func nextResolverKey() uintptr {
	resolverMu.Lock()
	defer resolverMu.Unlock()
	resolverKey++
	if resolverKey == 0 {
		resolverKey++
	}
	return resolverKey
}

//export wikitextGoResolve
func wikitextGoResolve(ctx unsafe.Pointer, name *C.uint8_t, nameLength C.size_t, body C.WikitextTemplateWriter) C.int32_t {
	key := uintptr(ctx)
	if key == 0 {
		return C.WIKITEXT_ERROR_NULL
	}
	value, found := resolvers.Load(key)
	if !found {
		return C.WIKITEXT_ERROR_NULL
	}
	resolver, ok := value.(Resolver)
	if !ok {
		return C.WIKITEXT_ERROR_ARGUMENT
	}
	requested := C.GoStringN((*C.char)(unsafe.Pointer(name)), C.int(nameLength))
	text, exists := resolver(requested)
	if !exists {
		return C.WIKITEXT_ERROR_NULL
	}
	bytes := []byte(text)
	if len(bytes) == 0 {
		return C.WIKITEXT_OK
	}
	return C.wikitextCallTemplateBody(
		body, nil, (*C.uint8_t)(unsafe.Pointer(&bytes[0])), C.size_t(len(bytes)),
	)
}

func optionsHandle(options *Options) *C.WikitextOptions {
	if options == nil {
		return nil
	}
	return options.handle
}

func Compile(source string, options *Options) (string, error) {
	bytes := []byte(source)
	var buffer C.WikitextOwnedBuffer
	status := C.wikitext_compile(
		bytesPointer(bytes), C.size_t(len(bytes)), optionsHandle(options), &buffer,
	)
	if status != C.WIKITEXT_OK {
		return "", statusError(status)
	}
	return takeBuffer(&buffer), nil
}

func Expand(source string, options *Options) (string, error) {
	bytes := []byte(source)
	var buffer C.WikitextOwnedBuffer
	status := C.wikitext_expand(
		bytesPointer(bytes), C.size_t(len(bytes)), optionsHandle(options), &buffer,
	)
	if status != C.WIKITEXT_OK {
		return "", statusError(status)
	}
	return takeBuffer(&buffer), nil
}

type Format int

const (
	HTML     Format = C.WIKITEXT_FORMAT_HTML
	Markdown Format = C.WIKITEXT_FORMAT_MARKDOWN
	Wikitext Format = C.WIKITEXT_FORMAT_WIKITEXT
)

func Convert(source string, from, to Format, options *Options) (string, error) {
	bytes := []byte(source)
	var buffer C.WikitextOwnedBuffer
	status := C.wikitext_convert(
		bytesPointer(bytes), C.size_t(len(bytes)),
		C.int32_t(from), C.int32_t(to),
		optionsHandle(options), &buffer,
	)
	if status != C.WIKITEXT_OK {
		return "", statusError(status)
	}
	return takeBuffer(&buffer), nil
}

func MarkdownToWikitext(source string) (string, error) {
	return Convert(source, Markdown, Wikitext, nil)
}

func HtmlToWikitext(source string) (string, error) {
	return Convert(source, HTML, Wikitext, nil)
}

func WikitextToMarkdown(source string) (string, error) {
	return Convert(source, Wikitext, Markdown, nil)
}

func HtmlToMarkdown(source string) (string, error) {
	return Convert(source, HTML, Markdown, nil)
}

func DecodeEntities(source string) (string, error) {
	bytes := []byte(source)
	var buffer C.WikitextOwnedBuffer
	status := C.wikitext_decode_entities(bytesPointer(bytes), C.size_t(len(bytes)), &buffer)
	if status != C.WIKITEXT_OK {
		return "", statusError(status)
	}
	return takeBuffer(&buffer), nil
}

type Heading struct {
	Level int
	ID    string
	Title string
}

type Category struct {
	Name    string
	SortKey string
}

type Diagnostic struct {
	Severity string
	Code     string
	Message  string
	Line     uint32
	Column   uint32
}

type Metadata struct {
	Title           string
	Redirect        string
	DisplayTitle    string
	DefaultSort     string
	PlainText       string
	Headings        []Heading
	Categories      []Category
	Diagnostics     []Diagnostic
	ReferenceCount  int
	ExternalLinks   int
	TOCNone         bool
	TOCForce        bool
	TOCInline       bool
	TrailingNewline bool
}

type Document struct {
	handle *C.WikitextDocument
	html   string
}

func Parse(source string, options *Options) (*Document, error) {
	bytes := []byte(source)
	var handle *C.WikitextDocument
	status := C.wikitext_parse(
		bytesPointer(bytes), C.size_t(len(bytes)), optionsHandle(options), &handle,
	)
	if status != C.WIKITEXT_OK {
		return nil, statusError(status)
	}
	document := &Document{handle: handle}
	var buffer C.WikitextOwnedBuffer
	if C.wikitext_document_html(handle, &buffer) == C.WIKITEXT_OK {
		document.html = takeBuffer(&buffer)
	}
	return document, nil
}

func (d *Document) Close() {
	if d == nil || d.handle == nil {
		return
	}
	C.wikitext_document_free(d.handle)
	d.handle = nil
}

func (d *Document) HTML() string {
	if d == nil {
		return ""
	}
	return d.html
}

func (d *Document) Metadata() (Metadata, error) {
	var metadata Metadata
	if d == nil || d.handle == nil {
		return metadata, ErrNull
	}
	var raw C.WikitextMetadata
	if status := C.wikitext_document_metadata(d.handle, &raw); status != C.WIKITEXT_OK {
		return metadata, statusError(status)
	}
	metadata.Title = takeOptional(&raw.title)
	metadata.Redirect = takeOptional(&raw.redirect)
	metadata.DisplayTitle = takeOptional(&raw.display_title)
	metadata.DefaultSort = takeOptional(&raw.default_sort)
	metadata.PlainText = takeOptional(&raw.plain_text)
	metadata.ReferenceCount = int(raw.reference_count)
	metadata.ExternalLinks = int(raw.external_link_count)
	metadata.TOCNone = bool(raw.toc_none)
	metadata.TOCForce = bool(raw.toc_force)
	metadata.TOCInline = bool(raw.toc_inline)
	metadata.TrailingNewline = bool(raw.trailing_newline)

	for index := range int(raw.heading_count) {
		var heading C.WikitextHeading
		if C.wikitext_document_heading(d.handle, C.size_t(index), &heading) != C.WIKITEXT_OK {
			continue
		}
		metadata.Headings = append(metadata.Headings, Heading{
			Level: int(heading.level),
			ID:    takeOptional(&heading.id),
			Title: takeOptional(&heading.title),
		})
	}
	for index := range int(raw.category_count) {
		var category C.WikitextCategory
		if C.wikitext_document_category(d.handle, C.size_t(index), &category) != C.WIKITEXT_OK {
			continue
		}
		metadata.Categories = append(metadata.Categories, Category{
			Name:    takeOptional(&category.name),
			SortKey: takeOptional(&category.sort_key),
		})
	}
	for index := range int(raw.diagnostic_count) {
		var diagnostic C.WikitextDiagnostic
		if C.wikitext_document_diagnostic(d.handle, C.size_t(index), &diagnostic) != C.WIKITEXT_OK {
			continue
		}
		metadata.Diagnostics = append(metadata.Diagnostics, Diagnostic{
			Severity: severityName(diagnostic.severity),
			Code:     takeOptional(&diagnostic.code),
			Message:  takeOptional(&diagnostic.message),
			Line:     uint32(diagnostic.line),
			Column:   uint32(diagnostic.column),
		})
	}
	return metadata, nil
}

func takeOptional(buffer **C.WikitextOwnedBuffer) string {
	if buffer == nil || *buffer == nil {
		return ""
	}
	text := takeBuffer(*buffer)
	*buffer = nil
	return text
}

func severityName(severity C.int32_t) string {
	switch severity {
	case C.WIKITEXT_SEVERITY_NOTE:
		return "note"
	case C.WIKITEXT_SEVERITY_WARNING:
		return "warning"
	case C.WIKITEXT_SEVERITY_ERROR:
		return "error"
	}
	return fmt.Sprintf("unknown(%d)", int(severity))
}

func CompileWith(source string, configure func(*Options) error) (string, error) {
	options, err := NewOptions()
	if err != nil {
		return "", err
	}
	defer options.Close()
	if configure != nil {
		if err := configure(options); err != nil {
			return "", err
		}
	}
	return Compile(source, options)
}
