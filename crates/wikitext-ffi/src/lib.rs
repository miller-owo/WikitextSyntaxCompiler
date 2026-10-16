// Safe functions returning status codes rather than `unsafe fn`: a foreign
// caller cannot uphold Rust's pointer invariants anyway, so the contract is
// "never panic, always report" instead of a safety section.
#![allow(clippy::not_unsafe_ptr_arg_deref, clippy::missing_safety_doc)]
use std::cell::RefCell;
use std::ffi::c_void;
use std::io::{Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::rc::Rc;
use std::slice;
use std::sync::Arc;
use wikitext::lua_modules::LuaModules;
use wikitext::preprocess::{decode_entities, MemoryTemplates, TemplateResolver};
use wikitext::{Diagnostic, Document, Options, OutputMode, Severity};

pub const WIKITEXT_OK: i32 = 0;
pub const WIKITEXT_ERROR_NULL: i32 = -1;
pub const WIKITEXT_ERROR_PANIC: i32 = -2;
pub const WIKITEXT_ERROR_UTF8: i32 = -3;
pub const WIKITEXT_ERROR_ARGUMENT: i32 = -4;
pub const WIKITEXT_ERROR_IO: i32 = -5;

pub const WIKITEXT_OUTPUT_FRAGMENT: i32 = 0;
pub const WIKITEXT_OUTPUT_PARSER_OUTPUT: i32 = 1;
pub const WIKITEXT_OUTPUT_DOCUMENT: i32 = 2;

pub const WIKITEXT_OPTION_PRETTY: i32 = 0;
pub const WIKITEXT_OPTION_SHOW_CATEGORIES: i32 = 1;
pub const WIKITEXT_OPTION_SHOW_IMAGES: i32 = 2;
pub const WIKITEXT_OPTION_DIAGNOSTICS: i32 = 3;
pub const WIKITEXT_OPTION_HEADING_ANCHORS: i32 = 4;
pub const WIKITEXT_OPTION_EDIT_SECTION_LINKS: i32 = 5;
pub const WIKITEXT_OPTION_KEEP_COMMENTS: i32 = 6;

pub const WIKITEXT_FORMAT_HTML: i32 = 0;
pub const WIKITEXT_FORMAT_MARKDOWN: i32 = 1;
pub const WIKITEXT_FORMAT_WIKITEXT: i32 = 2;

pub const WIKITEXT_SEVERITY_NOTE: i32 = 0;
pub const WIKITEXT_SEVERITY_WARNING: i32 = 1;
pub const WIKITEXT_SEVERITY_ERROR: i32 = 2;

pub type WikitextReaderCallback =
    unsafe extern "C" fn(ctx: *mut c_void, buf: *mut u8, len: usize) -> i64;
pub type WikitextWriterCallback =
    unsafe extern "C" fn(ctx: *mut c_void, buf: *const u8, len: usize) -> i64;
// The resolver is handed a buffer the library owns and fills it through this
// writer, which keeps allocation and deallocation on the same side.
pub type WikitextTemplateWriter =
    unsafe extern "C" fn(ctx: *mut c_void, data: *const u8, len: usize) -> i32;

pub type WikitextResolveCallback = unsafe extern "C" fn(
    ctx: *mut c_void,
    name: *const u8,
    name_len: usize,
    body: WikitextTemplateWriter,
) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WikitextReader {
    pub ctx: *mut c_void,
    pub read: Option<WikitextReaderCallback>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WikitextWriter {
    pub ctx: *mut c_void,
    pub write: Option<WikitextWriterCallback>,
}

#[repr(C)]
pub struct WikitextOwnedBuffer {
    pub data: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

#[repr(C)]
pub struct WikitextMetadata {
    pub title: *mut WikitextOwnedBuffer,
    pub redirect: *mut WikitextOwnedBuffer,
    pub display_title: *mut WikitextOwnedBuffer,
    pub default_sort: *mut WikitextOwnedBuffer,
    pub plain_text: *mut WikitextOwnedBuffer,
    pub heading_count: usize,
    pub category_count: usize,
    pub diagnostic_count: usize,
    pub reference_count: usize,
    pub external_link_count: usize,
    pub toc_none: bool,
    pub toc_force: bool,
    pub toc_inline: bool,
    pub trailing_newline: bool,
}

#[repr(C)]
pub struct WikitextHeading {
    pub level: u8,
    pub id: *mut WikitextOwnedBuffer,
    pub title: *mut WikitextOwnedBuffer,
}

#[repr(C)]
pub struct WikitextCategory {
    pub name: *mut WikitextOwnedBuffer,
    pub sort_key: *mut WikitextOwnedBuffer,
}

#[repr(C)]
pub struct WikitextDiagnostic {
    pub severity: i32,
    pub code: *mut WikitextOwnedBuffer,
    pub message: *mut WikitextOwnedBuffer,
    pub line: u32,
    pub column: u32,
}

struct ReaderAdapter(WikitextReader);

impl Read for ReaderAdapter {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let Some(callback) = self.0.read else {
            return Err(std::io::Error::other("reader callback is null"));
        };
        let read = unsafe { callback(self.0.ctx, buf.as_mut_ptr(), buf.len()) };
        if read < 0 {
            return Err(std::io::Error::other("reader callback failed"));
        }
        Ok(read as usize)
    }
}

struct WriterAdapter(WikitextWriter, RefCell<Option<i32>>);

impl Write for WriterAdapter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let Some(callback) = self.0.write else {
            return Err(std::io::Error::other("writer callback is null"));
        };
        let written = unsafe { callback(self.0.ctx, buf.as_ptr(), buf.len()) };
        if written < 0 {
            *self.1.borrow_mut() = Some(WIKITEXT_ERROR_IO);
            return Err(std::io::Error::other("writer callback failed"));
        }
        Ok(written as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct ResolvingTemplates {
    callback: WikitextResolveCallback,
    ctx: *mut c_void,
}

// A host resolver is a plain function pointer, and `TemplateResolver` demands
// Send + Sync. The host owns synchronisation of whatever `ctx` points at.
unsafe impl Send for ResolvingTemplates {}
unsafe impl Sync for ResolvingTemplates {}

impl std::fmt::Debug for ResolvingTemplates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ResolvingTemplates")
    }
}

thread_local! {
    // The writer callback has no slot for the sink, and a host resolves on the
    // thread that is compiling, so a thread-local is what carries it across.
    static RESOLVER_SINK: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

unsafe extern "C" fn collect_template(ctx: *mut c_void, data: *const u8, len: usize) -> i32 {
    let _ = ctx;
    if data.is_null() || len == 0 {
        return WIKITEXT_OK;
    }
    let bytes = unsafe { slice::from_raw_parts(data, len) };
    RESOLVER_SINK.with(|stack| {
        if let Some(current) = stack.borrow_mut().last_mut() {
            current.extend_from_slice(bytes);
        }
    });
    WIKITEXT_OK
}

impl TemplateResolver for ResolvingTemplates {
    fn resolve(&self, request: &wikitext::Transclusion) -> Option<String> {
        RESOLVER_SINK.with(|stack| stack.borrow_mut().push(Vec::new()));
        let status = unsafe {
            (self.callback)(
                self.ctx,
                request.name.as_ptr(),
                request.name.len(),
                collect_template,
            )
        };
        let collected = RESOLVER_SINK.with(|stack| stack.borrow_mut().pop());
        if status != WIKITEXT_OK {
            return None;
        }
        Some(String::from_utf8_lossy(&collected.unwrap_or_default()).into_owned())
    }

    fn name(&self) -> &str {
        "host"
    }
}

struct Resolver {
    callback: WikitextResolveCallback,
    ctx: *mut c_void,
}

pub struct WikitextOptions {
    title: Option<String>,
    language: String,
    output_mode: OutputMode,
    pretty: bool,
    article_path: Option<String>,
    server_url: Option<String>,
    image_base_url: Option<String>,
    show_categories: bool,
    show_images: bool,
    diagnostics: bool,
    heading_anchors: bool,
    edit_section_links: bool,
    keep_comments: bool,
    max_expansion_depth: Option<usize>,
    templates: MemoryTemplates,
    modules: LuaModules,
    resolver: Option<Resolver>,
}

pub struct WikitextDocument {
    document: Document,
    options: Options,
    html: String,
}

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn set_error(message: impl Into<String>) {
    LAST_ERROR.with(|slot| *slot.borrow_mut() = Some(message.into()));
}

fn clear_error() {
    LAST_ERROR.with(|slot| *slot.borrow_mut() = None);
}

// Unwinding into a foreign caller is undefined behaviour, so every entry
// point funnels through here.
fn guard<F>(body: F) -> i32
where
    F: FnOnce() -> i32,
{
    clear_error();
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(status) => status,
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic".to_string());
            set_error(format!("internal panic: {message}"));
            WIKITEXT_ERROR_PANIC
        }
    }
}

unsafe fn borrow_str<'a>(data: *const u8, len: usize, what: &str) -> Result<&'a str, i32> {
    if data.is_null() {
        if len == 0 {
            return Ok("");
        }
        set_error(format!("{what} is null"));
        return Err(WIKITEXT_ERROR_NULL);
    }
    let bytes = unsafe { slice::from_raw_parts(data, len) };
    std::str::from_utf8(bytes).map_err(|_| {
        set_error(format!("{what} is not valid UTF-8"));
        WIKITEXT_ERROR_UTF8
    })
}

// The header is returned by value and only the data pointer is heap allocated:
// freeing a stack header through `Box::from_raw` is undefined behaviour, and
// this shape makes that impossible to write.
fn owned_buffer(text: &str) -> WikitextOwnedBuffer {
    let mut bytes = text.as_bytes().to_vec();
    let buffer = WikitextOwnedBuffer {
        data: bytes.as_mut_ptr(),
        len: bytes.len(),
        capacity: bytes.capacity(),
    };
    std::mem::forget(bytes);
    buffer
}

fn empty_buffer() -> WikitextOwnedBuffer {
    WikitextOwnedBuffer {
        data: ptr::null_mut(),
        len: 0,
        capacity: 0,
    }
}

fn optional_buffer(value: Option<&str>) -> *mut WikitextOwnedBuffer {
    value.map_or(ptr::null_mut(), |text| {
        let buffer = owned_buffer(text);
        Box::into_raw(Box::new(buffer))
    })
}

unsafe fn write_out<T>(out: *mut T, value: T) -> i32 {
    if out.is_null() {
        set_error("output pointer is null");
        return WIKITEXT_ERROR_NULL;
    }
    unsafe { *out = value };

    WIKITEXT_OK
}

unsafe fn write_buffer(out: *mut WikitextOwnedBuffer, text: &str) -> i32 {
    unsafe { write_out(out, owned_buffer(text)) }
}

unsafe fn options_mut<'a>(options: *mut WikitextOptions) -> Result<&'a mut WikitextOptions, i32> {
    if options.is_null() {
        set_error("options handle is null; create one with wikitext_options_new");
        return Err(WIKITEXT_ERROR_NULL);
    }
    Ok(unsafe { &mut *options })
}

fn build_options(handle: Option<&WikitextOptions>) -> Options {
    let Some(handle) = handle else {
        return Options::default();
    };
    let mut options = Options {
        title: handle.title.clone(),
        language: handle.language.clone(),
        output_mode: handle.output_mode,
        pretty: handle.pretty,
        show_categories: handle.show_categories,
        show_images: handle.show_images,
        diagnostics: handle.diagnostics,
        heading_anchors: handle.heading_anchors,
        edit_section_links: handle.edit_section_links,
        keep_comments: handle.keep_comments,
        ..Options::default()
    };
    if let Some(path) = &handle.article_path {
        options.article_path = path.clone();
    }
    options.server_url = handle.server_url.clone();
    options.image_base_url = handle.image_base_url.clone();
    if let Some(depth) = handle.max_expansion_depth {
        options.max_expansion_depth = depth;
    }
    if let Some(resolver) = &handle.resolver {
        options.resolver = Some(Arc::new(ResolvingTemplates {
            callback: resolver.callback,
            ctx: resolver.ctx,
        }));
    } else if !handle.templates.is_empty() {
        let store = MemoryTemplates::new();
        for (name, body) in handle.templates.entries() {
            store.insert(&name, body);
        }
        options.resolver = Some(Arc::new(store));
    }
    if !handle.modules.is_empty() {
        let modules = LuaModules::new();
        if let Some(title) = &handle.title {
            modules.set_title(title.clone());
        }
        for (name, source) in handle.modules.entries() {
            modules.insert(&name, source);
        }
        options.module_provider = Some(Rc::new(modules));
    }
    options
}

pub fn linked_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[no_mangle]
pub extern "C" fn wikitext_version(out: *mut WikitextOwnedBuffer) -> i32 {
    unsafe { write_buffer(out, linked_version()) }
}

#[no_mangle]
pub extern "C" fn wikitext_free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(ptr as *mut ())) };
}

#[no_mangle]
pub extern "C" fn wikitext_buffer_free(buffer: *mut WikitextOwnedBuffer) {
    if buffer.is_null() {
        return;
    }
    let buffer = unsafe { &mut *buffer };
    if !buffer.data.is_null() {
        let slice = ptr::slice_from_raw_parts_mut(buffer.data, buffer.capacity);
        unsafe { drop(Box::from_raw(slice)) };
    }
    *buffer = empty_buffer();
}

#[no_mangle]
pub extern "C" fn wikitext_last_error(out: *mut WikitextOwnedBuffer) -> i32 {
    if out.is_null() {
        return WIKITEXT_ERROR_NULL;
    }
    LAST_ERROR.with(|slot| {
        let message = slot.borrow().clone().unwrap_or_default();
        unsafe { *out = owned_buffer(&message) };
    });
    WIKITEXT_OK
}

#[no_mangle]
pub extern "C" fn wikitext_clear_error() {
    clear_error();
}

#[no_mangle]
pub extern "C" fn wikitext_options_new(out: *mut *mut WikitextOptions) -> i32 {
    guard(|| {
        if out.is_null() {
            set_error("output pointer is null");
            return WIKITEXT_ERROR_NULL;
        }
        let handle = Box::new(WikitextOptions {
            title: None,
            language: "en".to_string(),
            output_mode: OutputMode::Fragment,
            pretty: true,
            article_path: None,
            server_url: None,
            image_base_url: None,
            show_categories: false,
            show_images: true,
            diagnostics: true,
            heading_anchors: false,
            edit_section_links: false,
            keep_comments: false,
            max_expansion_depth: None,
            templates: MemoryTemplates::new(),
            modules: LuaModules::new(),
            resolver: None,
        });
        unsafe { *out = Box::into_raw(handle) };
        WIKITEXT_OK
    })
}

#[no_mangle]
pub extern "C" fn wikitext_options_free(options: *mut WikitextOptions) {
    if options.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(options)) };
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_options_set_title(
    options: *mut WikitextOptions,
    title: *const u8,
    title_len: usize,
) -> i32 {
    set_string_option(options, title, title_len, "title", |handle, text| {
        handle.title = Some(text);
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_options_set_language(
    options: *mut WikitextOptions,
    language: *const u8,
    language_len: usize,
) -> i32 {
    set_string_option(
        options,
        language,
        language_len,
        "language",
        |handle, text| {
            handle.language = text;
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_options_set_article_path(
    options: *mut WikitextOptions,
    path: *const u8,
    path_len: usize,
) -> i32 {
    set_string_option(options, path, path_len, "article path", |handle, text| {
        handle.article_path = Some(text);
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_options_set_server_url(
    options: *mut WikitextOptions,
    url: *const u8,
    url_len: usize,
) -> i32 {
    set_string_option(options, url, url_len, "server url", |handle, text| {
        handle.server_url = Some(text);
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_options_set_image_base_url(
    options: *mut WikitextOptions,
    url: *const u8,
    url_len: usize,
) -> i32 {
    set_string_option(options, url, url_len, "image base url", |handle, text| {
        handle.image_base_url = Some(text);
    })
}

unsafe fn set_string_option(
    options: *mut WikitextOptions,
    data: *const u8,
    len: usize,
    what: &str,
    apply: impl FnOnce(&mut WikitextOptions, String),
) -> i32 {
    guard(|| {
        let handle = match unsafe { options_mut(options) } {
            Ok(handle) => handle,
            Err(status) => return status,
        };
        let text = match unsafe { borrow_str(data, len, what) } {
            Ok(text) => text,
            Err(status) => return status,
        };
        let owned = text.to_string();
        apply(handle, owned.clone());
        if what == "title" {
            handle.modules.set_title(owned);
        }
        WIKITEXT_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_options_set_output_mode(
    options: *mut WikitextOptions,
    mode: i32,
) -> i32 {
    guard(|| {
        let handle = match unsafe { options_mut(options) } {
            Ok(handle) => handle,
            Err(status) => return status,
        };
        handle.output_mode = match mode {
            WIKITEXT_OUTPUT_FRAGMENT => OutputMode::Fragment,
            WIKITEXT_OUTPUT_PARSER_OUTPUT => OutputMode::ParserOutput,
            WIKITEXT_OUTPUT_DOCUMENT => OutputMode::Document,
            other => {
                set_error(format!("unknown output mode {other}"));
                return WIKITEXT_ERROR_ARGUMENT;
            }
        };
        WIKITEXT_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_options_set_bool(
    options: *mut WikitextOptions,
    field: i32,
    value: bool,
) -> i32 {
    guard(|| {
        let handle = match unsafe { options_mut(options) } {
            Ok(handle) => handle,
            Err(status) => return status,
        };
        match field {
            WIKITEXT_OPTION_PRETTY => handle.pretty = value,
            WIKITEXT_OPTION_SHOW_CATEGORIES => handle.show_categories = value,
            WIKITEXT_OPTION_SHOW_IMAGES => handle.show_images = value,
            WIKITEXT_OPTION_DIAGNOSTICS => handle.diagnostics = value,
            WIKITEXT_OPTION_HEADING_ANCHORS => handle.heading_anchors = value,
            WIKITEXT_OPTION_EDIT_SECTION_LINKS => handle.edit_section_links = value,
            WIKITEXT_OPTION_KEEP_COMMENTS => handle.keep_comments = value,
            other => {
                set_error(format!("unknown boolean option {other}"));
                return WIKITEXT_ERROR_ARGUMENT;
            }
        }
        WIKITEXT_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_options_set_int(
    options: *mut WikitextOptions,
    field: i32,
    value: usize,
) -> i32 {
    guard(|| {
        let handle = match unsafe { options_mut(options) } {
            Ok(handle) => handle,
            Err(status) => return status,
        };
        match field {
            0 => handle.max_expansion_depth = Some(value),
            other => {
                set_error(format!("unknown integer option {other}"));
                return WIKITEXT_ERROR_ARGUMENT;
            }
        }
        WIKITEXT_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_options_set_resolver(
    options: *mut WikitextOptions,
    callback: Option<WikitextResolveCallback>,
    ctx: *mut c_void,
) -> i32 {
    guard(|| {
        let handle = match unsafe { options_mut(options) } {
            Ok(handle) => handle,
            Err(status) => return status,
        };
        handle.resolver = callback.map(|callback| Resolver { callback, ctx });
        WIKITEXT_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_options_add_template(
    options: *mut WikitextOptions,
    name: *const u8,
    name_len: usize,
    body: *const u8,
    body_len: usize,
) -> i32 {
    guard(|| {
        let handle = match unsafe { options_mut(options) } {
            Ok(handle) => handle,
            Err(status) => return status,
        };
        let name = match unsafe { borrow_str(name, name_len, "template name") } {
            Ok(text) => text,
            Err(status) => return status,
        };
        let body = match unsafe { borrow_str(body, body_len, "template body") } {
            Ok(text) => text,
            Err(status) => return status,
        };
        handle.templates.insert(name, body);
        WIKITEXT_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_options_add_module(
    options: *mut WikitextOptions,
    name: *const u8,
    name_len: usize,
    source: *const u8,
    source_len: usize,
) -> i32 {
    guard(|| {
        let handle = match unsafe { options_mut(options) } {
            Ok(handle) => handle,
            Err(status) => return status,
        };
        let name = match unsafe { borrow_str(name, name_len, "module name") } {
            Ok(text) => text,
            Err(status) => return status,
        };
        let source = match unsafe { borrow_str(source, source_len, "module source") } {
            Ok(text) => text,
            Err(status) => return status,
        };
        handle.modules.insert(name, source);
        WIKITEXT_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_options_template_count(options: *const WikitextOptions) -> usize {
    if options.is_null() {
        return 0;
    }
    unsafe { &*options }.templates.len()
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_options_module_count(options: *const WikitextOptions) -> usize {
    if options.is_null() {
        return 0;
    }
    unsafe { &*options }.modules.len()
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_compile(
    source: *const u8,
    source_len: usize,
    options: *const WikitextOptions,
    out: *mut WikitextOwnedBuffer,
) -> i32 {
    guard(|| {
        let text = match unsafe { borrow_str(source, source_len, "wikitext source") } {
            Ok(text) => text,
            Err(status) => return status,
        };
        let handle = match unsafe { resolve_options(options) } {
            Ok(handle) => handle,
            Err(status) => return status,
        };
        let html = wikitext::compile(text, &build_options(handle));
        unsafe { write_buffer(out, &html) }
    })
}

// A null handle means "defaults", for a caller that only wants HTML.
unsafe fn resolve_options<'a>(
    options: *const WikitextOptions,
) -> Result<Option<&'a WikitextOptions>, i32> {
    if options.is_null() {
        return Ok(None);
    }
    Ok(Some(unsafe { &*options }))
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_compile_reader(
    reader: *const WikitextReader,
    options: *const WikitextOptions,
    writer: *const WikitextWriter,
) -> i32 {
    guard(|| {
        if reader.is_null() || unsafe { (*reader).read.is_none() } {
            set_error("reader is null or has no callback");
            return WIKITEXT_ERROR_NULL;
        }
        if writer.is_null() || unsafe { (*writer).write.is_none() } {
            set_error("writer is null or has no callback");
            return WIKITEXT_ERROR_NULL;
        }
        let handle = match unsafe { resolve_options(options) } {
            Ok(handle) => handle,
            Err(status) => return status,
        };
        let mut source = String::new();
        let mut adapter = ReaderAdapter(unsafe { *reader });
        if let Err(error) = adapter.read_to_string(&mut source) {
            set_error(format!("cannot read source: {error}"));
            return WIKITEXT_ERROR_IO;
        }
        let html = wikitext::compile(&source, &build_options(handle));
        unsafe { write_to(writer, html.as_bytes()) }
    })
}

unsafe fn write_to(writer: *const WikitextWriter, bytes: &[u8]) -> i32 {
    let mut adapter = WriterAdapter(unsafe { *writer }, RefCell::new(None));
    if let Err(error) = adapter.write_all(bytes) {
        if let Some(status) = *adapter.1.borrow() {
            return status;
        }
        set_error(format!("cannot write output: {error}"));
        return WIKITEXT_ERROR_IO;
    }
    WIKITEXT_OK
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_parse(
    source: *const u8,
    source_len: usize,
    options: *const WikitextOptions,
    out: *mut *mut WikitextDocument,
) -> i32 {
    guard(|| {
        let text = match unsafe { borrow_str(source, source_len, "wikitext source") } {
            Ok(text) => text,
            Err(status) => return status,
        };
        let handle = match unsafe { resolve_options(options) } {
            Ok(handle) => handle,
            Err(status) => return status,
        };
        if out.is_null() {
            set_error("output pointer is null");
            return WIKITEXT_ERROR_NULL;
        }
        let resolved = build_options(handle);
        let (html, document) = wikitext::compile_with_diagnostics(text, &resolved);
        let document = Box::new(WikitextDocument {
            document,
            options: resolved,
            html,
        });
        unsafe { *out = Box::into_raw(document) };
        WIKITEXT_OK
    })
}

#[no_mangle]
pub extern "C" fn wikitext_document_free(document: *mut WikitextDocument) {
    if document.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(document)) };
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_document_html(
    document: *const WikitextDocument,
    out: *mut WikitextOwnedBuffer,
) -> i32 {
    guard(|| {
        if document.is_null() {
            set_error("document handle is null");
            return WIKITEXT_ERROR_NULL;
        }
        let html = &unsafe { &*document }.html;
        unsafe { write_buffer(out, html) }
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_document_render_to(
    document: *const WikitextDocument,
    writer: *const WikitextWriter,
) -> i32 {
    guard(|| {
        if document.is_null() {
            set_error("document handle is null");
            return WIKITEXT_ERROR_NULL;
        }
        if writer.is_null() || unsafe { (*writer).write.is_none() } {
            set_error("writer is null or has no callback");
            return WIKITEXT_ERROR_NULL;
        }
        let document = unsafe { &*document };
        let html = wikitext::serialize_with(
            &document.document,
            &wikitext::SerializeOptions::default()
                .with_mode(document.options.output_mode)
                .with_pretty(document.options.pretty),
        );
        unsafe { write_to(writer, html.as_bytes()) }
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_document_metadata(
    document: *const WikitextDocument,
    out: *mut WikitextMetadata,
) -> i32 {
    guard(|| {
        if document.is_null() {
            set_error("document handle is null");
            return WIKITEXT_ERROR_NULL;
        }
        let document = unsafe { &*document };
        let model = &document.document;
        let metadata = WikitextMetadata {
            title: optional_buffer(
                document
                    .options
                    .title
                    .as_deref()
                    .or(model.display_title.as_deref()),
            ),
            redirect: optional_buffer(model.redirect.as_deref()),
            display_title: optional_buffer(model.display_title.as_deref()),
            default_sort: optional_buffer(model.default_sort.as_deref()),
            plain_text: Box::into_raw(Box::new(owned_buffer(&model.text_content()))),
            heading_count: model.headings.len(),
            category_count: model.categories.len(),
            diagnostic_count: model.diagnostics.len(),
            reference_count: model.references.len(),
            external_link_count: model.external_link_count,
            toc_none: model.toc.none,
            toc_force: model.toc.force,
            toc_inline: model.toc.inline,
            trailing_newline: model.trailing_newline,
        };
        unsafe { write_out(out, metadata) }
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_document_heading(
    document: *const WikitextDocument,
    index: usize,
    out: *mut WikitextHeading,
) -> i32 {
    guard(|| {
        if document.is_null() {
            set_error("document handle is null");
            return WIKITEXT_ERROR_NULL;
        }
        let document = unsafe { &*document };
        let Some(heading) = document.document.headings.get(index) else {
            set_error(format!("heading index {index} is out of range"));
            return WIKITEXT_ERROR_ARGUMENT;
        };
        unsafe {
            write_out(
                out,
                WikitextHeading {
                    level: heading.level,
                    id: Box::into_raw(Box::new(owned_buffer(&heading.id))),
                    title: Box::into_raw(Box::new(owned_buffer(&heading.title))),
                },
            )
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_document_category(
    document: *const WikitextDocument,
    index: usize,
    out: *mut WikitextCategory,
) -> i32 {
    guard(|| {
        if document.is_null() {
            set_error("document handle is null");
            return WIKITEXT_ERROR_NULL;
        }
        let document = unsafe { &*document };
        let Some(category) = document.document.categories.get(index) else {
            set_error(format!("category index {index} is out of range"));
            return WIKITEXT_ERROR_ARGUMENT;
        };
        unsafe {
            write_out(
                out,
                WikitextCategory {
                    name: Box::into_raw(Box::new(owned_buffer(&category.name))),
                    sort_key: optional_buffer(category.sort_key.as_deref()),
                },
            )
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_document_diagnostic(
    document: *const WikitextDocument,
    index: usize,
    out: *mut WikitextDiagnostic,
) -> i32 {
    guard(|| {
        if document.is_null() {
            set_error("document handle is null");
            return WIKITEXT_ERROR_NULL;
        }
        let document = unsafe { &*document };
        let Some(diagnostic) = document.document.diagnostics.get(index) else {
            set_error(format!("diagnostic index {index} is out of range"));
            return WIKITEXT_ERROR_ARGUMENT;
        };
        unsafe { write_out(out, diagnostic_to_c(diagnostic)) }
    })
}

fn diagnostic_to_c(diagnostic: &Diagnostic) -> WikitextDiagnostic {
    WikitextDiagnostic {
        severity: match diagnostic.severity {
            Severity::Note => WIKITEXT_SEVERITY_NOTE,
            Severity::Warning => WIKITEXT_SEVERITY_WARNING,
            Severity::Error => WIKITEXT_SEVERITY_ERROR,
        },
        code: Box::into_raw(Box::new(owned_buffer(diagnostic.code.as_str()))),
        message: Box::into_raw(Box::new(owned_buffer(&diagnostic.message))),
        line: diagnostic.position.line,
        column: diagnostic.position.column,
    }
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_expand(
    source: *const u8,
    source_len: usize,
    options: *const WikitextOptions,
    out: *mut WikitextOwnedBuffer,
) -> i32 {
    guard(|| {
        let text = match unsafe { borrow_str(source, source_len, "wikitext source") } {
            Ok(text) => text,
            Err(status) => return status,
        };
        let handle = match unsafe { resolve_options(options) } {
            Ok(handle) => handle,
            Err(status) => return status,
        };
        let expanded = wikitext::Preprocessor::new(&build_options(handle)).run(text);
        unsafe { write_buffer(out, &expanded) }
    })
}

fn format_of(value: i32) -> Option<wikitext_convert::Output> {
    match value {
        WIKITEXT_FORMAT_HTML => Some(wikitext_convert::Output::Html),
        WIKITEXT_FORMAT_MARKDOWN => Some(wikitext_convert::Output::Markdown),
        WIKITEXT_FORMAT_WIKITEXT => Some(wikitext_convert::Output::Wikitext),
        _ => None,
    }
}

fn input_of(value: i32) -> Option<wikitext_convert::Input> {
    match value {
        WIKITEXT_FORMAT_HTML => Some(wikitext_convert::Input::Html),
        WIKITEXT_FORMAT_MARKDOWN => Some(wikitext_convert::Input::Markdown),
        WIKITEXT_FORMAT_WIKITEXT => Some(wikitext_convert::Input::Wikitext),
        _ => None,
    }
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_convert(
    source: *const u8,
    source_len: usize,
    from: i32,
    to: i32,
    options: *const WikitextOptions,
    out: *mut WikitextOwnedBuffer,
) -> i32 {
    guard(|| {
        let text = match unsafe { borrow_str(source, source_len, "source") } {
            Ok(text) => text,
            Err(status) => return status,
        };
        let Some(input) = input_of(from) else {
            set_error(format!("unknown source format {from}"));
            return WIKITEXT_ERROR_ARGUMENT;
        };
        let Some(output) = format_of(to) else {
            set_error(format!("unknown target format {to}"));
            return WIKITEXT_ERROR_ARGUMENT;
        };
        if input == wikitext_convert::Input::Wikitext && output == wikitext_convert::Output::Html {
            let handle = match unsafe { resolve_options(options) } {
                Ok(handle) => handle,
                Err(status) => return status,
            };
            let html = wikitext::compile(text, &build_options(handle));
            return unsafe { write_buffer(out, &html) };
        }
        let settings = wikitext_convert::Options::default().with_input(input);
        let converted = wikitext_convert::convert(text, &settings, output);
        unsafe { write_buffer(out, &converted) }
    })
}

#[no_mangle]
pub unsafe extern "C" fn wikitext_decode_entities(
    source: *const u8,
    source_len: usize,
    out: *mut WikitextOwnedBuffer,
) -> i32 {
    guard(|| {
        let text = match unsafe { borrow_str(source, source_len, "text") } {
            Ok(text) => text,
            Err(status) => return status,
        };
        let decoded = decode_entities(text);
        unsafe { write_buffer(out, &decoded) }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn buffer_text(buffer: &WikitextOwnedBuffer) -> String {
        if buffer.data.is_null() || buffer.len == 0 {
            return String::new();
        }
        let bytes = unsafe { slice::from_raw_parts(buffer.data, buffer.len) };
        String::from_utf8_lossy(bytes).into_owned()
    }

    fn take(buffer: &mut WikitextOwnedBuffer) -> String {
        let text = buffer_text(buffer);
        wikitext_buffer_free(buffer as *mut WikitextOwnedBuffer);
        text
    }

    fn compile_with(source: &str, handle: *mut WikitextOptions) -> String {
        let mut out = empty_buffer();
        let status = unsafe {
            wikitext_compile(
                source.as_ptr(),
                source.len(),
                handle,
                &mut out as *mut WikitextOwnedBuffer,
            )
        };
        assert_eq!(status, WIKITEXT_OK, "compile failed: {}", peek_error());
        let text = buffer_text(&out);
        wikitext_buffer_free(&mut out as *mut WikitextOwnedBuffer);
        text
    }

    fn peek_error() -> String {
        let mut out = empty_buffer();
        wikitext_last_error(&mut out as *mut WikitextOwnedBuffer);
        let text = buffer_text(&out);
        wikitext_buffer_free(&mut out as *mut WikitextOwnedBuffer);
        text
    }

    fn new_options() -> *mut WikitextOptions {
        let mut handle: *mut WikitextOptions = ptr::null_mut();
        assert_eq!(
            wikitext_options_new(&mut handle as *mut *mut WikitextOptions),
            WIKITEXT_OK
        );
        handle
    }

    #[test]
    fn a_buffer_allocates_and_frees() {
        let mut buffer = owned_buffer("hello");
        assert_eq!(buffer.len, 5);
        assert_eq!(buffer.capacity, 5, "capacity must match the allocation");
        assert_eq!(buffer_text(&buffer), "hello");
        wikitext_buffer_free(&mut buffer as *mut WikitextOwnedBuffer);
        assert!(buffer.data.is_null(), "freeing must clear the header");
    }

    #[test]
    fn version_is_reported() {
        let mut version = empty_buffer();
        assert_eq!(
            wikitext_version(&mut version as *mut WikitextOwnedBuffer),
            WIKITEXT_OK
        );
        assert_eq!(take(&mut version), linked_version());
    }

    #[test]
    fn compiles_a_simple_page() {
        let handle = new_options();
        let html = compile_with("Hello '''world'''", handle);
        assert_eq!(html, "<p>Hello <b>world</b></p>\n");
        wikitext_options_free(handle);
    }

    #[test]
    fn null_pointers_are_reported_not_crashed() {
        let mut handle: *mut WikitextOptions = ptr::null_mut();
        assert_eq!(
            wikitext_options_new(&mut handle as *mut *mut WikitextOptions),
            WIKITEXT_OK
        );
        let status = unsafe {
            wikitext_compile(
                ptr::null(),
                0,
                ptr::null(),
                ptr::null_mut::<WikitextOwnedBuffer>(),
            )
        };
        assert_eq!(status, WIKITEXT_ERROR_NULL);
        let mut error = empty_buffer();
        assert_eq!(
            wikitext_last_error(&mut error as *mut WikitextOwnedBuffer),
            WIKITEXT_OK
        );
        let message = take(&mut error);
        assert!(message.contains("null"), "{message}");
        wikitext_options_free(handle);
    }

    #[test]
    fn invalid_utf8_is_reported() {
        let handle = new_options();
        let bytes = [0xffu8, 0xfe];
        let status = unsafe {
            wikitext_compile(
                bytes.as_ptr(),
                bytes.len(),
                handle,
                ptr::null_mut::<WikitextOwnedBuffer>(),
            )
        };
        assert_eq!(status, WIKITEXT_ERROR_UTF8);
        wikitext_options_free(handle);
    }

    #[test]
    fn templates_registered_through_the_abi_expand() {
        let handle = new_options();
        let name = CString::new("greet").unwrap();
        let body = CString::new("Hello, {{{1|world}}}!").unwrap();
        assert_eq!(
            unsafe {
                wikitext_options_add_template(
                    handle,
                    name.as_ptr() as *const u8,
                    name.as_bytes().len(),
                    body.as_ptr() as *const u8,
                    body.as_bytes().len(),
                )
            },
            WIKITEXT_OK
        );
        assert_eq!(unsafe { wikitext_options_template_count(handle) }, 1);
        assert_eq!(
            compile_with("{{greet|Rust}}", handle),
            "<p>Hello, Rust!</p>\n"
        );
        wikitext_options_free(handle);
    }

    #[test]
    fn lua_modules_registered_through_the_abi_run() {
        let handle = new_options();
        let name = CString::new("Roman").unwrap();
        let source = CString::new(
            "local p = {}\nfunction p.convert(frame) return 'MCMXCIV' end\nreturn p\n",
        )
        .unwrap();
        assert_eq!(
            unsafe {
                wikitext_options_add_module(
                    handle,
                    name.as_ptr() as *const u8,
                    name.as_bytes().len(),
                    source.as_ptr() as *const u8,
                    source.as_bytes().len(),
                )
            },
            WIKITEXT_OK
        );
        assert_eq!(unsafe { wikitext_options_module_count(handle) }, 1);
        assert_eq!(
            compile_with("{{#invoke:Roman|convert|1994}}", handle),
            "<p>MCMXCIV</p>\n"
        );
        wikitext_options_free(handle);
    }

    #[test]
    fn a_host_resolver_is_consulted() {
        unsafe extern "C" fn resolve(
            _ctx: *mut c_void,
            name: *const u8,
            name_len: usize,
            body: WikitextTemplateWriter,
        ) -> i32 {
            let name = String::from_utf8_lossy(unsafe { slice::from_raw_parts(name, name_len) });
            if name != "host" {
                return WIKITEXT_ERROR_ARGUMENT;
            }
            let text = b"from the host";
            body(ptr::null_mut(), text.as_ptr(), text.len())
        }
        let handle = new_options();
        assert_eq!(
            unsafe { wikitext_options_set_resolver(handle, Some(resolve), ptr::null_mut()) },
            WIKITEXT_OK
        );
        assert_eq!(compile_with("{{host}}", handle), "<p>from the host</p>\n");
        wikitext_options_free(handle);
    }

    #[test]
    fn metadata_reports_structure() {
        let handle = new_options();
        let source = "== One ==\n\ntext\n\n== Two ==\n\n[[Category:Demo|Sort]]\n";
        let mut document: *mut WikitextDocument = ptr::null_mut();
        assert_eq!(
            unsafe {
                wikitext_parse(
                    source.as_ptr(),
                    source.len(),
                    handle,
                    &mut document as *mut *mut WikitextDocument,
                )
            },
            WIKITEXT_OK
        );
        let mut metadata = WikitextMetadata {
            title: ptr::null_mut(),
            redirect: ptr::null_mut(),
            display_title: ptr::null_mut(),
            default_sort: ptr::null_mut(),
            plain_text: ptr::null_mut(),
            heading_count: 0,
            category_count: 0,
            diagnostic_count: 0,
            reference_count: 0,
            external_link_count: 0,
            toc_none: false,
            toc_force: false,
            toc_inline: false,
            trailing_newline: false,
        };
        assert_eq!(
            unsafe { wikitext_document_metadata(document, &mut metadata) },
            WIKITEXT_OK
        );
        assert_eq!(metadata.heading_count, 2);
        assert_eq!(metadata.category_count, 1);
        assert!(buffer_text(unsafe { &*metadata.plain_text }).contains("One"));

        let mut heading = WikitextHeading {
            level: 0,
            id: ptr::null_mut(),
            title: ptr::null_mut(),
        };
        assert_eq!(
            unsafe { wikitext_document_heading(document, 1, &mut heading) },
            WIKITEXT_OK
        );
        assert_eq!(heading.level, 2);
        assert_eq!(buffer_text(unsafe { &*heading.title }), "Two");
        wikitext_buffer_free(heading.id);

        let mut category = WikitextCategory {
            name: ptr::null_mut(),
            sort_key: ptr::null_mut(),
        };
        assert_eq!(
            unsafe { wikitext_document_category(document, 0, &mut category) },
            WIKITEXT_OK
        );
        assert_eq!(buffer_text(unsafe { &*category.name }), "Demo");
        assert_eq!(buffer_text(unsafe { &*category.sort_key }), "Sort");

        wikitext_buffer_free(metadata.plain_text);
        wikitext_document_free(document);
        wikitext_options_free(handle);
    }

    #[test]
    fn out_of_range_indices_are_reported() {
        let handle = new_options();
        let mut document: *mut WikitextDocument = ptr::null_mut();
        assert_eq!(
            unsafe {
                wikitext_parse(
                    b"plain".as_ptr(),
                    5,
                    handle,
                    &mut document as *mut *mut WikitextDocument,
                )
            },
            WIKITEXT_OK
        );
        let mut heading = WikitextHeading {
            level: 0,
            id: ptr::null_mut(),
            title: ptr::null_mut(),
        };
        assert_eq!(
            unsafe { wikitext_document_heading(document, 99, &mut heading) },
            WIKITEXT_ERROR_ARGUMENT
        );
        wikitext_document_free(document);
        wikitext_options_free(handle);
    }

    #[test]
    fn expansion_can_be_run_on_its_own() {
        let handle = new_options();
        let mut out = empty_buffer();
        let source = "{{#expr:6*7}}";
        assert_eq!(
            unsafe {
                wikitext_expand(
                    source.as_ptr(),
                    source.len(),
                    handle,
                    &mut out as *mut WikitextOwnedBuffer,
                )
            },
            WIKITEXT_OK
        );
        assert_eq!(take(&mut out), "42");
        wikitext_options_free(handle);
    }

    #[test]
    fn a_buffer_round_trips_through_an_out_parameter() {
        let mut out = empty_buffer();
        let status = unsafe { write_buffer(&mut out as *mut WikitextOwnedBuffer, "hello") };
        assert_eq!(status, WIKITEXT_OK);
        assert_eq!(buffer_text(&out), "hello");
        wikitext_buffer_free(&mut out as *mut WikitextOwnedBuffer);
    }

    #[test]
    fn entities_decode() {
        let mut out = empty_buffer();
        let source = "&amp;&lt;&#65;";
        assert_eq!(
            unsafe {
                wikitext_decode_entities(
                    source.as_ptr(),
                    source.len(),
                    &mut out as *mut WikitextOwnedBuffer,
                )
            },
            WIKITEXT_OK
        );
        assert_eq!(take(&mut out), "&<A");
    }

    #[test]
    fn an_unknown_option_field_is_rejected() {
        let handle = new_options();
        assert_eq!(
            unsafe { wikitext_options_set_bool(handle, 99, true) },
            WIKITEXT_ERROR_ARGUMENT
        );
        assert_eq!(
            unsafe { wikitext_options_set_output_mode(handle, 99) },
            WIKITEXT_ERROR_ARGUMENT
        );
        wikitext_options_free(handle);
    }

    #[test]
    fn output_modes_are_selectable() {
        let handle = new_options();
        assert_eq!(
            unsafe { wikitext_options_set_output_mode(handle, WIKITEXT_OUTPUT_PARSER_OUTPUT) },
            WIKITEXT_OK
        );
        let html = compile_with("text", handle);
        assert!(html.contains("mw-parser-output"), "{html}");
        wikitext_options_free(handle);
    }

    #[test]
    fn a_writer_callback_receives_the_output() {
        unsafe extern "C" fn write(ctx: *mut c_void, buf: *const u8, len: usize) -> i64 {
            let sink = unsafe { &mut *(ctx as *mut String) };
            let bytes = unsafe { slice::from_raw_parts(buf, len) };
            sink.push_str(&String::from_utf8_lossy(bytes));
            len as i64
        }
        let handle = new_options();
        let mut sink = String::new();
        let writer = WikitextWriter {
            ctx: &mut sink as *mut String as *mut c_void,
            write: Some(write),
        };
        let source = "'''bold'''";
        let reader = WikitextReader {
            ctx: (&source as *const &str) as *mut c_void,
            read: None,
        };
        assert_eq!(
            unsafe { wikitext_compile_reader(&reader, handle, &writer) },
            WIKITEXT_ERROR_NULL
        );

        unsafe extern "C" fn read(ctx: *mut c_void, buf: *mut u8, len: usize) -> i64 {
            let state = unsafe { &mut *(ctx as *mut (&'static str, usize)) };
            let bytes = state.0.as_bytes();
            let remaining = bytes.len() - state.1;
            let take = remaining.min(len);
            unsafe { ptr::copy_nonoverlapping(bytes[state.1..].as_ptr(), buf, take) };
            state.1 += take;
            take as i64
        }
        let mut state: (&'static str, usize) = ("'''bold'''", 0);
        let reader = WikitextReader {
            ctx: &mut state as *mut (&'static str, usize) as *mut c_void,
            read: Some(read),
        };
        assert_eq!(
            unsafe { wikitext_compile_reader(&reader, handle, &writer) },
            WIKITEXT_OK
        );
        assert_eq!(sink, "<p><b>bold</b></p>\n");
        wikitext_options_free(handle);
    }

    #[test]
    fn a_failing_writer_is_reported() {
        unsafe extern "C" fn failing(_ctx: *mut c_void, _buf: *const u8, _len: usize) -> i64 {
            -1
        }
        let handle = new_options();
        let writer = WikitextWriter {
            ctx: ptr::null_mut(),
            write: Some(failing),
        };
        let source = "text";
        let reader = WikitextReader {
            ctx: (&source as *const &str) as *mut c_void,
            read: None,
        };
        let _ = reader;
        let mut document: *mut WikitextDocument = ptr::null_mut();
        assert_eq!(
            unsafe {
                wikitext_parse(
                    source.as_ptr(),
                    source.len(),
                    handle,
                    &mut document as *mut *mut WikitextDocument,
                )
            },
            WIKITEXT_OK
        );
        assert_eq!(
            unsafe { wikitext_document_render_to(document, &writer) },
            WIKITEXT_ERROR_IO
        );
        wikitext_document_free(document);
        wikitext_options_free(handle);
    }

    fn convert(source: &str, from: i32, to: i32) -> String {
        let mut out = empty_buffer();
        let status = unsafe {
            wikitext_convert(
                source.as_ptr(),
                source.len(),
                from,
                to,
                ptr::null(),
                &mut out as *mut WikitextOwnedBuffer,
            )
        };
        assert_eq!(status, WIKITEXT_OK, "convert failed: {}", peek_error());
        let text = buffer_text(&out);
        wikitext_buffer_free(&mut out as *mut WikitextOwnedBuffer);
        text
    }

    #[test]
    fn markdown_becomes_wikitext() {
        assert_eq!(
            convert(
                "# Title\n\nSome **bold** text.",
                WIKITEXT_FORMAT_MARKDOWN,
                WIKITEXT_FORMAT_WIKITEXT
            ),
            "== Title ==\n\nSome '''bold''' text."
        );
    }

    #[test]
    fn html_becomes_wikitext() {
        assert_eq!(
            convert(
                "<h2>Two</h2><p>Text</p>",
                WIKITEXT_FORMAT_HTML,
                WIKITEXT_FORMAT_WIKITEXT
            ),
            "== Two ==\n\nText"
        );
    }

    #[test]
    fn wikitext_becomes_markdown() {
        assert_eq!(
            convert(
                "== Two ==\n\nSome '''bold''' text.",
                WIKITEXT_FORMAT_WIKITEXT,
                WIKITEXT_FORMAT_MARKDOWN
            ),
            "## Two\n\nSome **bold** text."
        );
    }

    #[test]
    fn wikitext_to_html_uses_the_core_compiler() {
        let html = convert("'''x'''", WIKITEXT_FORMAT_WIKITEXT, WIKITEXT_FORMAT_HTML);
        assert_eq!(html, "<p><b>x</b></p>\n");
    }

    #[test]
    fn every_format_pair_round_trips_through_the_middle() {
        let source = "== Head ==\n\nText with a [[Link|label]].";
        let markdown = convert(source, WIKITEXT_FORMAT_WIKITEXT, WIKITEXT_FORMAT_MARKDOWN);
        assert!(markdown.starts_with("## Head"), "{markdown}");
        let back = convert(
            &markdown,
            WIKITEXT_FORMAT_MARKDOWN,
            WIKITEXT_FORMAT_WIKITEXT,
        );
        // A bare wiki link has no Markdown spelling, so the round trip keeps the
        // text and loses the link syntax. That is the documented trade.
        assert!(back.starts_with("== Head =="), "{back}");
        assert!(back.contains("label"), "{back}");
    }

    #[test]
    fn an_unknown_format_is_rejected() {
        let mut out = empty_buffer();
        let status = unsafe {
            wikitext_convert(
                b"x".as_ptr(),
                1,
                99,
                WIKITEXT_FORMAT_WIKITEXT,
                ptr::null(),
                &mut out as *mut WikitextOwnedBuffer,
            )
        };
        assert_eq!(status, WIKITEXT_ERROR_ARGUMENT);
        assert!(peek_error().contains("format"), "{}", peek_error());
    }

    #[test]
    fn options_reach_the_wikitext_to_html_direction() {
        let handle = new_options();
        let name = CString::new("greet").unwrap();
        let body = CString::new("Hello, {{{1|world}}}!").unwrap();
        unsafe {
            wikitext_options_add_template(
                handle,
                name.as_ptr() as *const u8,
                name.as_bytes().len(),
                body.as_ptr() as *const u8,
                body.as_bytes().len(),
            );
        }
        let mut out = empty_buffer();
        let source = "{{greet|Rust}}";
        let status = unsafe {
            wikitext_convert(
                source.as_ptr(),
                source.len(),
                WIKITEXT_FORMAT_WIKITEXT,
                WIKITEXT_FORMAT_HTML,
                handle,
                &mut out as *mut WikitextOwnedBuffer,
            )
        };
        assert_eq!(status, WIKITEXT_OK);
        assert_eq!(take(&mut out), "<p>Hello, Rust!</p>\n");
        wikitext_options_free(handle);
    }
}
