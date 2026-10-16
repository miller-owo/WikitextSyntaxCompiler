package wikitext

import (
	"errors"
	"strings"
	"testing"
)

func TestVersionIsReported(t *testing.T) {
	if version := Version(); version == "" {
		t.Fatal("Version() returned nothing")
	}
}

func TestCompilesASimplePage(t *testing.T) {
	html, err := Compile("Hello '''world'''", nil)
	if err != nil {
		t.Fatalf("Compile: %v", err)
	}
	if want := "<p>Hello <b>world</b></p>\n"; html != want {
		t.Errorf("got %q, want %q", html, want)
	}
}

func TestTemplatesExpand(t *testing.T) {
	options, err := NewOptions()
	if err != nil {
		t.Fatalf("NewOptions: %v", err)
	}
	defer options.Close()

	if err := options.AddTemplate("greet", "Hello, {{{1|world}}}!"); err != nil {
		t.Fatalf("AddTemplate: %v", err)
	}
	if options.TemplateCount() != 1 {
		t.Errorf("TemplateCount() = %d, want 1", options.TemplateCount())
	}
	html, err := Compile("{{greet|Rust}}", options)
	if err != nil {
		t.Fatalf("Compile: %v", err)
	}
	if want := "<p>Hello, Rust!</p>\n"; html != want {
		t.Errorf("got %q, want %q", html, want)
	}
}

func TestLuaModulesRun(t *testing.T) {
	options, err := NewOptions()
	if err != nil {
		t.Fatalf("NewOptions: %v", err)
	}
	defer options.Close()

	module := "local p = {}\nfunction p.convert(frame) return 'MCMXCIV' end\nreturn p\n"
	if err := options.AddModule("Roman", module); err != nil {
		t.Fatalf("AddModule: %v", err)
	}
	if options.ModuleCount() != 1 {
		t.Errorf("ModuleCount() = %d, want 1", options.ModuleCount())
	}
	html, err := Compile("{{#invoke:Roman|convert|1994}}", options)
	if err != nil {
		t.Fatalf("Compile: %v", err)
	}
	if want := "<p>MCMXCIV</p>\n"; html != want {
		t.Errorf("got %q, want %q", html, want)
	}
}

func TestResolverSuppliesTemplates(t *testing.T) {
	options, err := NewOptions()
	if err != nil {
		t.Fatalf("NewOptions: %v", err)
	}
	defer options.Close()

	store := map[string]string{"host": "from the host"}
	var asked []string
	err = options.SetResolver(func(name string) (string, bool) {
		asked = append(asked, name)
		body, found := store[name]
		return body, found
	})
	if err != nil {
		t.Fatalf("SetResolver: %v", err)
	}
	html, err := Compile("{{host}}", options)
	if err != nil {
		t.Fatalf("Compile: %v", err)
	}
	if want := "<p>from the host</p>\n"; html != want {
		t.Errorf("got %q, want %q", html, want)
	}
	if len(asked) == 0 || asked[0] != "host" {
		t.Errorf("the resolver saw %v, want the first name to be \"host\"", asked)
	}
}

func TestMissingTemplateIsNotAnError(t *testing.T) {
	html, err := Compile("{{absent}}", nil)
	if err != nil {
		t.Fatalf("a missing template must not fail the compile: %v", err)
	}
	// The name is normalised to a page title, so the first letter is uppercased.
	if !strings.Contains(html, "Absent") {
		t.Errorf("the red link should mention the name, got %q", html)
	}
}

func TestMetadataReportsStructure(t *testing.T) {
	options, err := NewOptions()
	if err != nil {
		t.Fatalf("NewOptions: %v", err)
	}
	defer options.Close()

	source := "== One ==\n\ntext\n\n=== Two ===\n\n[[Category:Demo|Sort]]\n"
	document, err := Parse(source, options)
	if err != nil {
		t.Fatalf("Parse: %v", err)
	}
	defer document.Close()

	if document.HTML() == "" {
		t.Error("Parse produced no HTML")
	}
	metadata, err := document.Metadata()
	if err != nil {
		t.Fatalf("Metadata: %v", err)
	}
	if len(metadata.Headings) != 2 {
		t.Fatalf("got %d headings, want 2", len(metadata.Headings))
	}
	if metadata.Headings[1].Level != 3 || metadata.Headings[1].Title != "Two" {
		t.Errorf("second heading = %+v, want level 3 titled Two", metadata.Headings[1])
	}
	if len(metadata.Categories) != 1 {
		t.Fatalf("got %d categories, want 1", len(metadata.Categories))
	}
	if metadata.Categories[0].Name != "Demo" || metadata.Categories[0].SortKey != "Sort" {
		t.Errorf("category = %+v", metadata.Categories[0])
	}
	if !strings.Contains(metadata.PlainText, "One") {
		t.Errorf("plain text = %q", metadata.PlainText)
	}
}

func TestOutputModes(t *testing.T) {
	options, err := NewOptions()
	if err != nil {
		t.Fatalf("NewOptions: %v", err)
	}
	defer options.Close()

	if err := options.SetOutputMode(ParserOutput); err != nil {
		t.Fatalf("SetOutputMode: %v", err)
	}
	html, err := Compile("text", options)
	if err != nil {
		t.Fatalf("Compile: %v", err)
	}
	if !strings.Contains(html, "mw-parser-output") {
		t.Errorf("parser-output mode should wrap the fragment, got %q", html)
	}
}

func TestInvalidUtf8IsReported(t *testing.T) {
	_, err := Compile("\xff\xfe", nil)
	if err == nil {
		t.Fatal("invalid UTF-8 should fail")
	}
	if !errors.Is(err, ErrUTF8) {
		t.Errorf("got %v, want ErrUTF8", err)
	}
}

func TestExpandAndDecode(t *testing.T) {
	expanded, err := Expand("{{#expr:6*7}}", nil)
	if err != nil {
		t.Fatalf("Expand: %v", err)
	}
	if expanded != "42" {
		t.Errorf("Expand = %q, want 42", expanded)
	}
	decoded, err := DecodeEntities("&amp;&lt;&#65;")
	if err != nil {
		t.Fatalf("DecodeEntities: %v", err)
	}
	if decoded != "&<A" {
		t.Errorf("DecodeEntities = %q, want &<A", decoded)
	}
}

func TestOptionsMethodsRejectAClosedHandle(t *testing.T) {
	options, err := NewOptions()
	if err != nil {
		t.Fatalf("NewOptions: %v", err)
	}
	options.Close()
	options.Close()
	if err := options.SetTitle("x"); !errors.Is(err, ErrNull) {
		t.Errorf("SetTitle after Close = %v, want ErrNull", err)
	}
	if err := options.AddTemplate("a", "b"); !errors.Is(err, ErrNull) {
		t.Errorf("AddTemplate after Close = %v, want ErrNull", err)
	}
}

func TestCompileWithConfigures(t *testing.T) {
	html, err := CompileWith("{{greet}}", func(options *Options) error {
		return options.AddTemplate("greet", "configured")
	})
	if err != nil {
		t.Fatalf("CompileWith: %v", err)
	}
	if want := "<p>configured</p>\n"; html != want {
		t.Errorf("got %q, want %q", html, want)
	}
}

func TestManyCompilesDoNotLeakHandles(t *testing.T) {
	// A handle table that never shrinks would still pass every other test.
	for index := range 50 {
		options, err := NewOptions()
		if err != nil {
			t.Fatalf("NewOptions: %v", err)
		}
		if err := options.SetResolver(func(string) (string, bool) { return "", false }); err != nil {
			t.Fatalf("SetResolver: %v", err)
		}
		if _, err := Compile("x", options); err != nil {
			t.Fatalf("Compile %d: %v", index, err)
		}
		options.Close()
	}
	count := 0
	resolvers.Range(func(_, _ any) bool {
		count++
		return true
	})
	if count != 0 {
		t.Errorf("%d resolver handles survived Close", count)
	}
}
func TestMarkdownBecomesWikitext(t *testing.T) {
	wikitext, err := MarkdownToWikitext("# Title\n\nSome **bold** text.")
	if err != nil {
		t.Fatalf("MarkdownToWikitext: %v", err)
	}
	if want := "== Title ==\n\nSome '''bold''' text."; wikitext != want {
		t.Errorf("got %q, want %q", wikitext, want)
	}
}

func TestHtmlBecomesWikitext(t *testing.T) {
	wikitext, err := HtmlToWikitext("<h2>Two</h2><p>Text</p>")
	if err != nil {
		t.Fatalf("HtmlToWikitext: %v", err)
	}
	if want := "== Two ==\n\nText"; wikitext != want {
		t.Errorf("got %q, want %q", wikitext, want)
	}
}

func TestWikitextBecomesMarkdown(t *testing.T) {
	markdown, err := WikitextToMarkdown("== Two ==\n\nSome '''bold''' text.")
	if err != nil {
		t.Fatalf("WikitextToMarkdown: %v", err)
	}
	if want := "## Two\n\nSome **bold** text."; markdown != want {
		t.Errorf("got %q, want %q", markdown, want)
	}
}

func TestConvertUsesOptionsForWikitextToHtml(t *testing.T) {
	options, err := NewOptions()
	if err != nil {
		t.Fatalf("NewOptions: %v", err)
	}
	defer options.Close()
	if err := options.AddTemplate("greet", "Hello, {{{1|world}}}!"); err != nil {
		t.Fatalf("AddTemplate: %v", err)
	}
	html, err := Convert("{{greet|Rust}}", Wikitext, HTML, options)
	if err != nil {
		t.Fatalf("Convert: %v", err)
	}
	if want := "<p>Hello, Rust!</p>\n"; html != want {
		t.Errorf("got %q, want %q", html, want)
	}
}

func TestConvertRejectsAnUnknownFormat(t *testing.T) {
	_, err := Convert("x", Format(99), Wikitext, nil)
	if err == nil {
		t.Fatal("an unknown format should fail")
	}
	if !errors.Is(err, ErrArgument) {
		t.Errorf("got %v, want ErrArgument", err)
	}
}

func TestEveryFormatPairConverts(t *testing.T) {
	source := "== Head ==\n\nText with a link."
	for _, from := range []Format{HTML, Markdown, Wikitext} {
		for _, to := range []Format{HTML, Markdown, Wikitext} {
			if _, err := Convert(source, from, to, nil); err != nil {
				t.Errorf("%v -> %v: %v", from, to, err)
			}
		}
	}
}
