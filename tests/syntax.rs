//! 001 T005 syntax units and search documents (context-v2 § Syntax units and
//! search documents): per mapped language a nested container, a function of
//! at least 5 lines and a wrapper yield the expected units (kind, qualified
//! name, byte range); documents tile the source and lie inside their delivery
//! units; malformed parses still tile; oversize parts stay within 4096 bytes;
//! unmapped and fence-only languages and oversize sources are blocks;
//! Markdown sections nest by rank and ignore fenced headings.
use context_foundry::syntax::{self, Lang, UnitKind};

/// `(kind, qualified name, exact source text of the unit's range)` in
/// source (pre-)order.
fn units(source: &str, lang: Lang) -> Vec<(&'static str, Option<String>, &str)> {
    syntax::units(source, lang)
        .into_iter()
        .map(|unit| {
            (
                unit.kind.as_str(),
                unit.qname,
                &source[unit.start..unit.end],
            )
        })
        .collect()
}

fn named<'a>(
    kind: &'static str,
    qname: &str,
    text: &'a str,
) -> (&'static str, Option<String>, &'a str) {
    (kind, Some(qname.to_owned()), text)
}

/// Text from `start` (inclusive) through the first `end` after it (inclusive).
fn span<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let from = source
        .find(start)
        .unwrap_or_else(|| panic!("{start:?} not in source"));
    let to = source[from..]
        .find(end)
        .unwrap_or_else(|| panic!("{end:?} not after {start:?}"))
        + from
        + end.len();
    &source[from..to]
}

/// The document invariants: ranges plus whitespace-only gaps tile
/// `[0, len)` without overlap; every document is non-whitespace, on UTF-8
/// boundaries and inside its delivery unit; parts of a split region are at
/// most 4096 bytes. Returns the documents.
fn assert_tiles(source: &str, lang: Option<Lang>) -> Vec<syntax::Document> {
    let documents = syntax::documents(source, lang);
    let mut cursor = 0;
    for document in &documents {
        assert!(document.start < document.end, "{document:?}");
        assert!(cursor <= document.start, "overlap at {document:?}");
        assert!(
            source[cursor..document.start].trim().is_empty(),
            "a non-whitespace gap before {document:?}"
        );
        assert!(source.is_char_boundary(document.start) && source.is_char_boundary(document.end));
        assert!(!source[document.start..document.end].trim().is_empty());
        assert!(
            document.unit.start <= document.start && document.end <= document.unit.end,
            "outside its delivery unit: {document:?}"
        );
        cursor = document.end;
    }
    assert!(
        source[cursor..].trim().is_empty(),
        "uncovered tail from {cursor}"
    );
    assert_eq!(syntax::documents(source, lang), documents, "deterministic");
    documents
}

#[test]
fn rust_units_nest_with_qualified_names() {
    let source = "mod outer {
    pub struct Point {
        x: i32,
    }

    impl Point {
        pub fn len(&self) -> i32 {
            let a = self.x;
            let b = a * 2;
            let c = b + 1;
            c
        }
    }
}

macro_rules! twice {
    ($e:expr) => { $e * 2 };
}

const LIMIT: usize = 4;
";
    assert_eq!(
        units(source, Lang::Rust),
        [
            named("mod", "outer", span(source, "mod outer", "    }\n}")),
            named(
                "struct",
                "outer::Point",
                span(source, "pub struct Point", "    }")
            ),
            named(
                "impl",
                "outer::Point",
                span(source, "impl Point", "        }\n    }")
            ),
            named(
                "fn",
                "outer::Point::len",
                span(source, "pub fn len", "        }")
            ),
            named("macro", "twice", span(source, "macro_rules!", "};\n}")),
            named("const", "LIMIT", "const LIMIT: usize = 4;"),
        ]
    );
    let documents = assert_tiles(source, Some(Lang::Rust));
    // Leaves are their own delivery unit; container residuals are delivered
    // as the container.
    let len = documents
        .iter()
        .find(|d| source[d.start..d.end].contains("let b"))
        .unwrap();
    assert_eq!(len.unit.qname.as_deref(), Some("outer::Point::len"));
    assert_eq!((len.start, len.end), (len.unit.start, len.unit.end));
    let residual = documents
        .iter()
        .find(|d| source[d.start..d.end].starts_with("mod outer {"))
        .unwrap();
    assert_eq!(residual.unit.kind, UnitKind::Mod);
}

#[test]
fn python_decorators_supply_the_range() {
    let source = r#"class Shape:
    """Doc."""

    @staticmethod
    def area(w, h):
        total = w * h
        half = total / 2
        double = half * 4
        return double / 2


@decorator
def helper():
    return 1
"#;
    assert_eq!(
        units(source, Lang::Python),
        [
            named(
                "class",
                "Shape",
                span(source, "class Shape", "return double / 2")
            ),
            named(
                "fn",
                "Shape.area",
                span(source, "@staticmethod", "return double / 2")
            ),
            named("fn", "helper", span(source, "@decorator", "return 1")),
        ]
    );
    assert_tiles(source, Some(Lang::Python));
}

#[test]
fn typescript_exports_supply_the_range_and_arrow_constants_are_functions() {
    let source = r#"export class Store {
  get(key: string): string {
    const a = key;
    const b = a + "x";
    const c = b + "y";
    return c;
  }
}

export interface Shape {
  area(): number;
}

type Id = string;

enum Color { Red, Green }

export const make = (n: number): number => {
  return n + 1;
};

function plain() {}
"#;
    assert_eq!(
        units(source, Lang::TypeScript),
        [
            named(
                "class",
                "Store",
                span(source, "export class Store", "  }\n}")
            ),
            named("method", "Store.get", span(source, "get(key", "  }")),
            named("interface", "Shape", span(source, "export interface", "}")),
            named("type", "Id", "type Id = string;"),
            named("enum", "Color", "enum Color { Red, Green }"),
            named("variant", "Color.Red", "Red"),
            named("variant", "Color.Green", "Green"),
            named("fn", "make", span(source, "export const make", "};")),
            named("fn", "plain", "function plain() {}"),
        ]
    );
    assert_tiles(source, Some(Lang::TypeScript));
}

#[test]
fn tsx_units_parse_jsx_bodies() {
    let source = "export function View(props: Props) {
  const a = 1;
  const b = 2;
  const c = 3;
  return <div>{a + b + c}</div>;
}

class Box extends Base {
  render() {
    return <span />;
  }
}
";
    assert_eq!(
        units(source, Lang::Tsx),
        [
            named(
                "fn",
                "View",
                span(source, "export function View", "</div>;\n}")
            ),
            named("class", "Box", span(source, "class Box", "  }\n}")),
            named("method", "Box.render", span(source, "render()", "  }")),
        ]
    );
    assert_tiles(source, Some(Lang::Tsx));
}

#[test]
fn javascript_units_need_exactly_one_function_declarator() {
    let source = "export default class Widget {
  draw(ctx) {
    ctx.save();
    ctx.fill();
    ctx.stroke();
    ctx.restore();
  }
}

function* gen() { yield 1; }

const handler = function (e) { return e; };

let a = 1, b = () => 2;
";
    assert_eq!(
        units(source, Lang::JavaScript),
        [
            named("class", "Widget", span(source, "export default", "  }\n}")),
            named("method", "Widget.draw", span(source, "draw(ctx)", "  }")),
            named("fn", "gen", "function* gen() { yield 1; }"),
            named(
                "fn",
                "handler",
                "const handler = function (e) { return e; };"
            ),
            // Module-level declarators are units each (context-v2 § Unit
            // kinds, amended for 001 T007): a function value is `fn`.
            named("static", "a", "a = 1"),
            named("fn", "b", "b = () => 2"),
        ]
    );
    assert_tiles(source, Some(Lang::JavaScript));
}

#[test]
fn go_types_methods_and_functions_are_named_units() {
    let source = "package shapes

type Point struct {
	X int
}

func (p Point) Len() int {
	a := p.X
	b := a * 2
	c := b + 1
	return c
}

func Helper() {}
";
    assert_eq!(
        units(source, Lang::Go),
        [
            // A Go type spec is named (amended for 001 T007).
            named("type", "Point", span(source, "type Point", "}")),
            named(
                "method",
                "Len",
                span(source, "func (p Point)", "return c\n}")
            ),
            named("fn", "Helper", "func Helper() {}"),
        ]
    );
    assert_tiles(source, Some(Lang::Go));
}

/// Rust 2024 `safe` foreign items: tree-sitter-rust 0.24 has no `safe`
/// keyword, but its error recovery keeps the item a signature or static item
/// with its name, so each is a unit. Items inside a macro body such as
/// `cfg_select! { … }` are token trees, not units.
#[test]
fn rust_safe_foreign_items_are_units_outside_macro_bodies() {
    let source = "unsafe extern \"C\" {
    pub(crate) safe fn asinf(x: f32) -> f32;
    pub safe static FLAG: u8;
}
cfg_select! {
    _ => { unsafe extern \"C\" { pub safe fn acosf(x: f32) -> f32; } }
}
";
    assert_eq!(
        units(source, Lang::Rust),
        [
            named("fn", "asinf", "pub(crate) safe fn asinf(x: f32) -> f32;"),
            named("static", "FLAG", "pub safe static FLAG: u8;"),
        ]
    );
}

#[test]
fn c_units_need_a_body_and_take_the_innermost_declarator() {
    let source = "struct outer {
    struct inner { int y; } in;
};

int *make(int n) {
    int a = n;
    int b = a * 2;
    int c = b + 1;
    return 0;
}

struct outer origin;
";
    assert_eq!(
        units(source, Lang::C),
        [
            named(
                "struct",
                "outer",
                span(source, "struct outer {", "} in;\n}")
            ),
            named("struct", "outer.inner", "struct inner { int y; }"),
            named("fn", "make", span(source, "int *make", "return 0;\n}")),
        ]
    );
    assert_tiles(source, Some(Lang::C));
}

#[test]
fn parenthesized_c_and_cpp_declarators_name_the_innermost_identifier() {
    // `(probe)` is a parenthesized declarator; `getter` returns a pointer to
    // a function, so its identifier sits inside a parenthesized pointer
    // declarator wrapping a function declarator.
    let source = "int (probe)(void) { return 1; }\n\nint (*getter(void))(int) { return 0; }\n";
    for lang in [Lang::C, Lang::Cpp] {
        assert_eq!(
            units(source, lang),
            [
                named("fn", "probe", "int (probe)(void) { return 1; }"),
                named("fn", "getter", "int (*getter(void))(int) { return 0; }"),
            ],
            "{lang:?}"
        );
        assert_tiles(source, Some(lang));
    }
}

#[test]
fn deeply_nested_units_build_documents_on_a_two_mib_stack() {
    // 65,536 nested anonymous structs: about 640 KiB, under the 1 MiB parse
    // limit. The 2 MiB stack is the default of the runtime's blocking worker
    // threads, where indexing builds documents.
    const DEPTH: usize = 65_536;
    let source = format!("{}int x;{}", "struct {".repeat(DEPTH), "};".repeat(DEPTH));
    assert!(source.len() < 1024 * 1024);
    let (units, documents) = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let units = syntax::units(&source, Lang::C);
            (units.len(), assert_tiles(&source, Some(Lang::C)))
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(units, DEPTH, "every nesting level is a unit");
    // Each level but the innermost has a `struct {` residual before its child
    // and a `;}` residual after it; the innermost is one leaf document; the
    // final `;` is a block.
    assert_eq!(documents.len(), 2 * (DEPTH - 1) + 1 + 1);
    assert_eq!(documents[0].start..documents[0].end, 0..8);
    assert_eq!(documents[0].unit.start, 0);
}

#[test]
fn qualified_names_keep_their_last_256_bytes_from_a_utf8_boundary() {
    // `é` is two bytes. The middle module's full name is 30 `é`, `::`, 200
    // `a`: 262 bytes, so its last 256 bytes keep 27 `é` (a cut on a boundary).
    // `f`'s full name adds `::f`: 265 bytes. Its last 256 bytes would start
    // inside the fifth `é`, so the kept tail starts at the next boundary and
    // keeps 25 `é`; building it from the middle module's kept name agrees.
    let outer = "é".repeat(30);
    let inner = "a".repeat(200);
    let source = format!("mod {outer} {{\n    mod {inner} {{\n        fn f() {{}}\n    }}\n}}\n");
    let qnames: Vec<Option<String>> = syntax::units(&source, Lang::Rust)
        .into_iter()
        .map(|unit| unit.qname)
        .collect();
    let middle = format!("{}::{inner}", "é".repeat(27));
    let tail = format!("{}::{inner}::f", "é".repeat(25));
    assert_eq!((middle.len(), tail.len()), (256, 255));
    assert_eq!(qnames, [Some(outer), Some(middle), Some(tail)]);
}

#[test]
fn deeply_nested_named_units_keep_bounded_qualified_names_on_a_two_mib_stack() {
    // 65,536 nested `namespace n {`: 917,510 bytes, under the 1 MiB parse
    // limit. Full qualified names would total about 6 GiB; each kept name is
    // the last 256 bytes of `n::n::…::n`.
    const DEPTH: usize = 65_536;
    let source = format!(
        "{}int x;{}",
        "namespace n {".repeat(DEPTH),
        "}".repeat(DEPTH)
    );
    assert_eq!(source.len(), 917_510);
    let (units, documents) = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let units = syntax::units(&source, Lang::Cpp);
            let documents = assert_tiles(&source, Some(Lang::Cpp));
            (units, documents)
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(units.len(), DEPTH, "every nesting level is a unit");
    // The join of 100 names is 298 bytes; its last 256 bytes equal the last
    // 256 bytes of any deeper chain. Depth d (0-based) has 3d + 1 full bytes.
    let chain = vec!["n"; 100].join("::");
    let deep = &chain[chain.len() - 256..];
    for (depth, unit) in units.iter().enumerate() {
        let qname = unit.qname.as_deref();
        if 3 * depth + 1 > 256 {
            assert_eq!(qname, Some(deep), "depth {depth}");
        } else {
            let full = vec!["n"; depth + 1].join("::");
            assert_eq!(qname, Some(full.as_str()), "depth {depth}");
        }
    }
    // Each level but the innermost has a `namespace n {` residual before its
    // child and a `}` residual after it; the innermost is one leaf document.
    assert_eq!(documents.len(), 2 * (DEPTH - 1) + 1);
    assert!(documents.iter().all(|document| {
        document
            .unit
            .qname
            .as_deref()
            .is_some_and(|q| q.len() <= 256)
    }));
}

#[test]
fn cpp_templates_supply_the_range_and_names_use_double_colons() {
    let source = "namespace geo {
class Shape {
public:
    int area() const {
        int a = 1;
        int b = 2;
        int c = 3;
        return a + b + c;
    }
};
}

template <typename T>
T twice(T x) {
    return x * 2;
}

int geo::Shape::perimeter() const { return 0; }
";
    assert_eq!(
        units(source, Lang::Cpp),
        [
            named("mod", "geo", span(source, "namespace geo", "};\n}")),
            named(
                "class",
                "geo::Shape",
                span(source, "class Shape", "    }\n}")
            ),
            named(
                "fn",
                "geo::Shape::area",
                span(source, "int area()", "    }")
            ),
            named(
                "fn",
                "twice",
                span(source, "template <typename T>", "x * 2;\n}")
            ),
            named(
                "fn",
                "perimeter",
                "int geo::Shape::perimeter() const { return 0; }"
            ),
        ]
    );
    assert_tiles(source, Some(Lang::Cpp));
}

#[test]
fn java_members_nest_and_annotations_belong_to_the_declaration() {
    let source = "@Entity
public class Account {
    private int balance;

    public Account(int start) {
        balance = start;
    }

    public int deposit(int amount) {
        int a = amount;
        int b = a * 2;
        int c = b - a;
        balance += c;
        return balance;
    }

    interface Listener { void on(); }

    enum Kind { A, B }

    record Pair(int l, int r) {}
}
";
    assert_eq!(
        units(source, Lang::Java),
        [
            named("class", "Account", source.trim_end()),
            named(
                "method",
                "Account.Account",
                span(source, "public Account(", "    }")
            ),
            named(
                "method",
                "Account.deposit",
                span(source, "public int deposit", "    }")
            ),
            named(
                "interface",
                "Account.Listener",
                "interface Listener { void on(); }"
            ),
            named("method", "Account.Listener.on", "void on();"),
            named("enum", "Account.Kind", "enum Kind { A, B }"),
            named("variant", "Account.Kind.A", "A"),
            named("variant", "Account.Kind.B", "B"),
            named("class", "Account.Pair", "record Pair(int l, int r) {}"),
        ]
    );
    assert_tiles(source, Some(Lang::Java));
}

#[test]
fn markdown_sections_stop_at_equal_or_higher_rank_and_ignore_fenced_headings() {
    let source = "# Title

Intro.

## Part A

Text A.

```
# not a heading
```

### Deep

## Part B

# Second
";
    let from = |heading: &str| source.find(heading).unwrap();
    assert_eq!(
        units(source, Lang::Markdown),
        [
            named("section", "Title", &source[..from("# Second")]),
            named(
                "section",
                "Title.Part A",
                &source[from("## Part A")..from("## Part B")]
            ),
            named(
                "section",
                "Title.Part A.Deep",
                &source[from("### Deep")..from("## Part B")]
            ),
            named(
                "section",
                "Title.Part B",
                &source[from("## Part B")..from("# Second")]
            ),
            named("section", "Second", &source[from("# Second")..]),
        ]
    );
    assert_tiles(source, Some(Lang::Markdown));
}

#[test]
fn a_malformed_parse_still_yields_units_and_tiles() {
    let source = "fn broken( {\n    let x = ;\n}\n\nstruct Kept {\n    a: u8,\n}\n";
    let found = units(source, Lang::Rust);
    assert!(
        found.contains(&named("struct", "Kept", "struct Kept {\n    a: u8,\n}")),
        "{found:?}"
    );
    assert_tiles(source, Some(Lang::Rust));
}

#[test]
fn oversize_regions_split_into_parts_of_at_most_4096_bytes() {
    // A unit body over 8192 bytes splits at line boundaries; every part keeps
    // the unit as its delivery unit.
    let body: String = (0..600).map(|i| format!("    let v{i} = {i};\n")).collect();
    let source = format!("fn big() {{\n{body}}}\n");
    assert!(source.len() > 8192);
    let documents = assert_tiles(&source, Some(Lang::Rust));
    assert!(documents.len() > 2, "{}", documents.len());
    for document in &documents {
        assert!(document.end - document.start <= 4096);
        assert_eq!(document.unit.qname.as_deref(), Some("big"));
        assert_eq!(
            (document.unit.start, document.unit.end),
            (0, source.len() - 1)
        );
    }
    // A single line longer than 4096 bytes splits at UTF-8 boundaries.
    let line = "東京".repeat(3000);
    let source = format!("intro\n{line}\noutro\n");
    let documents = assert_tiles(&source, None);
    assert!(documents.iter().all(|d| d.end - d.start <= 4096));
    assert!(documents.len() >= 3);
}

#[test]
fn unmapped_fence_only_and_oversize_sources_are_blocks_merged_up_to_2048_bytes() {
    let paragraphs: String = (0..40)
        .map(|i| format!("paragraph {i} {}\n\n", "word ".repeat(18)))
        .collect();
    for (path, source) in [
        ("notes.txt", paragraphs.clone()),
        ("Makefile", paragraphs.clone()),
        (".bashrc", paragraphs.clone()),
        ("config.toml", "[a]\nb = 1\n\n[c]\nd = 2\n".to_owned()),
    ] {
        let lang = Lang::from_path(path);
        assert!(lang.is_none_or(|lang| !lang.has_units()), "{path}");
        let documents = assert_tiles(&source, lang);
        assert!(
            documents.iter().all(|d| d.unit.kind == UnitKind::Block
                && (d.unit.start, d.unit.end) == (d.start, d.end)),
            "{path}"
        );
        assert!(documents.iter().all(|d| d.end - d.start <= 2048), "{path}");
    }
    let merged = syntax::documents(&paragraphs, None);
    assert!(
        merged.len() > 1 && merged.len() < 40,
        "blank-line pieces merge up to 2048 bytes: {}",
        merged.len()
    );
    // A mapped source over 1 MiB is not parsed.
    let big = "fn f() {}\n".repeat(syntax::MAX_PARSE_BYTES / 10 + 1);
    assert!(syntax::units(&big, Lang::Rust).is_empty());
    assert!(
        assert_tiles(&big, Some(Lang::Rust))
            .iter()
            .all(|d| d.unit.kind == UnitKind::Block)
    );
}

#[test]
fn the_extension_map_selects_languages_and_fence_tags() {
    for (path, tag, has_units) in [
        ("a.rs", "rust", true),
        ("a.pyi", "python", true),
        ("a.cts", "typescript", true),
        ("a.tsx", "tsx", true),
        ("a.jsx", "javascript", true),
        ("a.go", "go", true),
        ("a.c", "c", true),
        ("a.h", "cpp", true),
        ("a.hxx", "cpp", true),
        ("src/A.java", "java", true),
        ("README.markdown", "markdown", true),
        ("a.yml", "yaml", false),
        ("a.sql", "sql", false),
        // 001 T008 (context-v2 § City map › Languages).
        ("a.cs", "csharp", true),
        ("a.fs", "fsharp", true),
        ("a.fsi", "fsharp", true),
        ("a.fsx", "fsharp", true),
        ("a.vb", "vbnet", true),
        ("a.php", "php", true),
        ("a.phtml", "php", true),
        ("a.pl", "perl", true),
        ("a.pm", "perl", true),
        ("t/basic.t", "perl", true),
        ("a.psgi", "perl", true),
        ("a.sh", "bash", true),
        ("a.bash", "bash", true),
        ("a.zsh", "bash", true),
        ("a.ps1", "powershell", true),
        ("a.psm1", "powershell", true),
        ("a.psd1", "powershell", true),
        ("a.rb", "ruby", true),
        ("lib/tasks/db.rake", "ruby", true),
        ("Rakefile", "ruby", true),
        ("app/Gemfile", "ruby", true),
        ("a.kt", "kotlin", true),
        ("build.gradle.kts", "kotlin", true),
        ("a.swift", "swift", true),
        ("a.scala", "scala", true),
        ("a.sc", "scala", true),
        ("a.lua", "lua", true),
        ("a.dart", "dart", true),
        ("a.ex", "elixir", true),
        ("a.exs", "elixir", true),
        ("a.hs", "haskell", true),
    ] {
        let lang = Lang::from_path(path).unwrap_or_else(|| panic!("{path} is mapped"));
        assert_eq!((lang.tag(), lang.has_units()), (tag, has_units), "{path}");
    }
    for unmapped in [
        "a.txt",
        "Makefile",
        ".rs",
        "dir.rs/file",
        "a.RS",
        "Gemfile.lock",
        "rakefile",
        "a.lhs",
    ] {
        assert_eq!(Lang::from_path(unmapped), None, "{unmapped}");
    }
    assert_eq!(
        Lang::from_path("a.fsi"),
        Some(Lang::FSharpSignature),
        "a signature file takes the signature grammar"
    );
}

// ---------------------------------------------------------------------------
// Outlines (001 T006)
// ---------------------------------------------------------------------------

/// Line start offsets: line `n` (1-based) starts at index `n - 1`.
fn line_starts(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(source.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}

/// Kept bytes plus the source lines each marker stands for.
fn reconstruct(source: &str, segments: &[syntax::Segment]) -> String {
    let starts = line_starts(source);
    let mut out = String::new();
    for segment in segments {
        match segment {
            syntax::Segment::Kept { start, end } => out.push_str(&source[*start..*end]),
            syntax::Segment::Elided {
                first_line,
                last_line,
                indent,
            } => {
                let from = starts[first_line - 1];
                let to = starts.get(*last_line).copied().unwrap_or(source.len());
                assert!(source[from..to].starts_with(indent.as_str()));
                out.push_str(&source[from..to]);
            }
        }
    }
    out
}

/// The 1-based lines touched by kept segments, and the marker count.
fn visible(
    source: &str,
    segments: &[syntax::Segment],
) -> (std::collections::BTreeSet<usize>, usize) {
    let starts = line_starts(source);
    let line_of = |at: usize| starts.partition_point(|&s| s <= at);
    let mut kept = std::collections::BTreeSet::new();
    let mut markers = 0;
    for segment in segments {
        match segment {
            syntax::Segment::Kept { start, end } => kept.extend(line_of(*start)..=line_of(end - 1)),
            syntax::Segment::Elided { .. } => markers += 1,
        }
    }
    (kept, markers)
}

#[test]
fn outline_min_folds_every_outermost_span_and_outline_unfolds_what_fits() {
    // A multi-line signature (lines 1-3) with a 4-line interior, then a
    // same-line-brace unit without an interior.
    let source = "fn alpha(\n    a: u8,\n) -> u8 {\n    let b = a;\n    let c = b;\n    let d = c;\n    d\n}\n\nfn beta() { 1 }\n";
    let whole = 0..source.len();
    let min = syntax::outline(source, Lang::Rust, whole.clone(), 0, 0);
    assert_eq!(
        syntax::render_outline(source, &min),
        "fn alpha(\n    a: u8,\n) -> u8 {\n    ⋯ 4-7\n}\n\nfn beta() { 1 }\n"
    );
    assert_eq!(reconstruct(source, &min), source);
    let full = syntax::outline(source, Lang::Rust, whole, 60, 120);
    assert_eq!(
        full,
        [syntax::Segment::Kept {
            start: 0,
            end: source.len()
        }]
    );
    // The signature form: the unit's own range, everything folded.
    let alpha_end = source.find("}\n").unwrap() + 1;
    assert_eq!(
        syntax::render_outline(
            source,
            &syntax::outline(source, Lang::Rust, 0..alpha_end, 0, 0)
        ),
        "fn alpha(\n    a: u8,\n) -> u8 {\n    ⋯ 4-7\n}"
    );
}

#[test]
fn unfolding_stops_at_the_target_and_skips_spans_that_overflow_the_limit() {
    // Two units, each with a 4-line interior: 13 lines, 7 visible folded.
    let unit = |name: &str| format!("fn {name}() {{\n    1;\n    2;\n    3;\n    4\n}}\n");
    let source = format!("{}\n{}", unit("a"), unit("b"));
    let whole = 0..source.len();
    let folded = |segments: &[syntax::Segment]| -> Vec<(usize, usize)> {
        segments
            .iter()
            .filter_map(|segment| match segment {
                syntax::Segment::Elided {
                    first_line,
                    last_line,
                    ..
                } => Some((*first_line, *last_line)),
                syntax::Segment::Kept { .. } => None,
            })
            .collect()
    };
    // Each unfold would reach 10 visible lines, over the limit of 9: both
    // are skipped.
    let skipped = syntax::outline(&source, Lang::Rust, whole.clone(), 9, 9);
    assert_eq!(folded(&skipped), [(2, 5), (9, 12)]);
    // With a limit of 10 the first unfold fits and reaches the target.
    let stopped = syntax::outline(&source, Lang::Rust, whole, 9, 10);
    assert_eq!(folded(&stopped), [(9, 12)]);
    for segments in [&skipped, &stopped] {
        assert_eq!(reconstruct(&source, segments), source);
    }
}

#[test]
fn a_nested_block_comment_folds_when_its_body_unfolds() {
    let source = "fn long() {\n    /*\n     * one\n     * two\n     * three\n     * four\n     */\n    let x = 1;\n    x\n}\n";
    let whole = 0..source.len();
    // The interior (2-9) unfolds; its 6-line comment stays folded because
    // unfolding it would exceed the limit of 8.
    assert_eq!(
        syntax::render_outline(
            source,
            &syntax::outline(source, Lang::Rust, whole.clone(), 60, 8)
        ),
        "fn long() {\n    ⋯ 2-7\n    let x = 1;\n    x\n}\n"
    );
    assert_eq!(
        syntax::render_outline(
            source,
            &syntax::outline(source, Lang::Rust, whole.clone(), 0, 0)
        ),
        "fn long() {\n    ⋯ 2-9\n}\n"
    );
    assert_eq!(
        syntax::outline(source, Lang::Rust, whole, 60, 120),
        [syntax::Segment::Kept {
            start: 0,
            end: source.len()
        }]
    );
}

#[test]
fn container_gaps_python_blocks_and_markdown_sections_elide_by_their_rules() {
    // A 5-line container gap (2-6) and a member's 4-line interior (8-11).
    let rust = "mod store {\n    use std::a;\n    use std::b;\n    use std::c;\n    use std::d;\n\n    pub fn get() -> u8 {\n        let a = 1;\n        let b = 2;\n        let c = 3;\n        a + b + c\n    }\n}\n";
    assert_eq!(
        syntax::render_outline(
            rust,
            &syntax::outline(rust, Lang::Rust, 0..rust.len(), 0, 0)
        ),
        "mod store {\n    ⋯ 2-6\n    pub fn get() -> u8 {\n        ⋯ 8-11\n    }\n}\n"
    );
    // Python: the signature runs through the `:`; a block has no closing
    // line; the class gap (2-3) is too short to elide.
    let python = "class Shape:\n    \"\"\"Doc.\"\"\"\n\n    def area(\n        self,\n    ):\n        a = 1\n        b = 2\n        c = 3\n        return a + b + c\n";
    assert_eq!(
        syntax::render_outline(
            python,
            &syntax::outline(python, Lang::Python, 0..python.len(), 0, 0)
        ),
        "class Shape:\n    \"\"\"Doc.\"\"\"\n\n    def area(\n        self,\n    ):\n        ⋯ 7-10\n"
    );
    // Markdown: section bodies elide whatever their length; subsection
    // headings stay.
    let markdown = "# Title\nintro one\nintro two\n## Sub\nsub text\n";
    assert_eq!(
        syntax::render_outline(
            markdown,
            &syntax::outline(markdown, Lang::Markdown, 0..markdown.len(), 0, 0)
        ),
        "# Title\n⋯ 2-3\n## Sub\n⋯ 5-5\n"
    );
    for (source, lang) in [
        (rust, Lang::Rust),
        (python, Lang::Python),
        (markdown, Lang::Markdown),
    ] {
        let segments = syntax::outline(source, lang, 0..source.len(), 0, 0);
        assert_eq!(reconstruct(source, &segments), source);
    }
}

/// A generated 331-line source, the 1-based lines that must stay visible
/// (every signature line and every closing line on its own line), and the
/// interior lines of the struct and of the first method.
fn generated_rust() -> (String, Vec<usize>, Vec<usize>) {
    let mut source = String::new();
    let mut mandatory = Vec::new();
    let mut first_interiors = Vec::new();
    let mut line = 0usize;
    let mut push = |source: &mut String, text: &str, keep: bool, first: bool| {
        source.push_str(text);
        source.push('\n');
        line += 1;
        if keep {
            mandatory.push(line);
        }
        if first {
            first_interiors.push(line);
        }
    };
    push(&mut source, "pub struct Store {", true, false);
    for i in 0..6 {
        push(&mut source, &format!("    field_{i}: u8,"), false, true);
    }
    push(&mut source, "}", true, false);
    push(&mut source, "", false, false);
    push(&mut source, "impl Store {", true, false);
    for i in 0..3 {
        // A multi-line signature, a 30-line interior.
        push(&mut source, &format!("    pub fn multi_{i}("), true, false);
        push(&mut source, "        &self,", true, false);
        push(&mut source, "        a: u8,", true, false);
        push(&mut source, "    ) -> u8 {", true, false);
        for j in 0..29 {
            push(
                &mut source,
                &format!("        let v{j} = a + {j};"),
                false,
                i == 0,
            );
        }
        push(&mut source, "        a", false, i == 0);
        push(&mut source, "    }", true, false);
        push(&mut source, "", false, false);
    }
    push(&mut source, "}", true, false);
    for i in 0..4 {
        push(&mut source, "", false, false);
        // A same-line-brace signature, a 50-line interior.
        push(
            &mut source,
            &format!("pub fn same_{i}(a: u8) -> u8 {{"),
            true,
            false,
        );
        for j in 0..49 {
            push(
                &mut source,
                &format!("    let w{j} = a * {j};"),
                false,
                false,
            );
        }
        push(&mut source, "    a", false, false);
        push(&mut source, "}", true, false);
    }
    (source, mandatory, first_interiors)
}

#[test]
fn outlines_of_a_300_line_file_keep_every_signature_and_closing_line() {
    let (source, mandatory, first_interiors) = generated_rust();
    assert_eq!(source.lines().count(), 331);
    let whole = 0..source.len();
    let min = syntax::outline(&source, Lang::Rust, whole.clone(), 0, 0);
    let outline = syntax::outline(&source, Lang::Rust, whole, 60, 120);
    for segments in [&min, &outline] {
        assert_eq!(reconstruct(&source, segments), source);
        let (kept, _) = visible(&source, segments);
        for line in &mandatory {
            assert!(kept.contains(line), "line {line} hidden");
        }
    }
    // outline-min: the 27 mandatory lines, 8 blank separators and one marker
    // per interior (the struct's fields, 3 methods, 4 functions): 43 lines.
    let blank: Vec<usize> = source
        .lines()
        .enumerate()
        .filter(|(_, text)| text.is_empty())
        .map(|(index, _)| index + 1)
        .collect();
    assert_eq!((mandatory.len(), blank.len()), (27, 8));
    let mut expected: std::collections::BTreeSet<usize> = mandatory.iter().copied().collect();
    expected.extend(blank.iter().copied());
    let (kept, markers) = visible(&source, &min);
    assert_eq!((kept.clone(), markers), (expected.clone(), 8));
    // outline (60/120): breadth-first in source order, the struct's fields
    // unfold (43 + 5 = 48 < 60), then the first method (48 + 29 = 77, within
    // 120), which reaches the target: 77 lines, 6 markers.
    expected.extend(first_interiors.iter().copied());
    let (kept, markers) = visible(&source, &outline);
    assert_eq!((kept.len() + markers, markers), (77, 6));
    assert_eq!(kept, expected);
}

#[test]
fn a_many_member_class_keeps_every_member_signature() {
    let mut source = String::from("@Entity\npublic class Ledger {\n");
    let mut signatures = vec![1usize, 2];
    for i in 0..40 {
        let line = source.lines().count();
        if i % 2 == 0 {
            source.push_str(&format!(
                "    @Override\n    public int total{i}(\n        int a,\n        int b) {{\n        int c = a + b;\n        int d = c * 2;\n        int e = d - 1;\n        return e;\n    }}\n"
            ));
            signatures.extend(line + 1..=line + 4);
            signatures.push(line + 9);
        } else {
            source.push_str(&format!(
                "    private int helper{i}(int a) {{\n        int b = a;\n        int c = b;\n        int d = c;\n        return d;\n    }}\n"
            ));
            signatures.push(line + 1);
            signatures.push(line + 6);
        }
    }
    source.push_str("}\n");
    signatures.push(source.lines().count());
    let min = syntax::outline(&source, Lang::Java, 0..source.len(), 0, 0);
    assert_eq!(reconstruct(&source, &min), source);
    let (kept, markers) = visible(&source, &min);
    assert_eq!(markers, 40, "one marker per member interior");
    let expected: std::collections::BTreeSet<usize> = signatures.into_iter().collect();
    assert_eq!(kept, expected);
}

#[test]
fn a_language_without_units_or_an_oversize_source_outlines_as_its_text() {
    let toml = "[a]\nb = 1\nc = 2\nd = 3\ne = 4\nf = 5\n";
    assert_eq!(
        syntax::outline(toml, Lang::Toml, 0..toml.len(), 0, 0),
        [syntax::Segment::Kept {
            start: 0,
            end: toml.len()
        }]
    );
    let body: String = (0..9000)
        .map(|i| format!("    let v{i} = {i};\n"))
        .collect();
    let big = format!("fn big() {{\n{body}}}\n");
    assert!(big.len() > 1024 * 1024 / 8 && big.len() < 1024 * 1024);
    let oversize = format!("{}{big}", " ".repeat(1024 * 1024));
    assert_eq!(
        syntax::outline(&oversize, Lang::Rust, 0..oversize.len(), 0, 0),
        [syntax::Segment::Kept {
            start: 0,
            end: oversize.len()
        }]
    );
}

/// The `(first, last)` lines of every elided run, in order.
fn elided(segments: &[syntax::Segment]) -> Vec<(usize, usize)> {
    segments
        .iter()
        .filter_map(|segment| match segment {
            syntax::Segment::Elided {
                first_line,
                last_line,
                ..
            } => Some((*first_line, *last_line)),
            syntax::Segment::Kept { .. } => None,
        })
        .collect()
}

#[test]
fn an_unfold_over_the_limit_is_skipped_and_later_spans_still_unfold() {
    // A 100-line interior (2-101), then a 4-line interior (105-108): 109
    // lines, 7 visible folded. Unfolding the first (+99) exceeds the limit of
    // 20 and is skipped; the second (+3) still fits and unfolds.
    let big: String = (0..100).map(|i| format!("    {i};\n")).collect();
    let source = format!("fn a() {{\n{big}}}\n\nfn b() {{\n    1;\n    2;\n    3;\n    4\n}}\n");
    let segments = syntax::outline(&source, Lang::Rust, 0..source.len(), 60, 20);
    assert_eq!(elided(&segments), [(2, 101)]);
    assert_eq!(reconstruct(&source, &segments), source);
}

#[test]
fn declaration_only_members_and_ancestor_signature_lines_are_never_elided() {
    // Interface members, trait signature items and C++ prototypes are not
    // units, yet every one of their lines is a signature.
    let typescript = format!(
        "interface Api {{\n{}}}\n",
        (0..8)
            .map(|i| format!("  method{i}(a: number): void;\n"))
            .collect::<String>()
    );
    let rust = "pub trait Store {\n    fn get(&self, key: &str) -> Option<String>;\n    fn put(&mut self, key: &str, value: String);\n    fn delete(&mut self, key: &str) -> bool;\n    fn len(&self) -> usize;\n}\n";
    let cpp = "class Api {\npublic:\n    int a();\n    int b();\n    int c();\n    int d();\n    int e();\n};\n";
    // A method of an object literal that is a default parameter lies on the
    // enclosing function's signature lines (1-8).
    let javascript = "function outer(options = {\n    inner() {\n        const a = 1;\n        const b = 2;\n        const c = 3;\n        return a + b + c;\n    },\n}) {\n    return options;\n}\n";
    for (source, lang) in [
        (typescript.as_str(), Lang::TypeScript),
        (rust, Lang::Rust),
        (cpp, Lang::Cpp),
        (javascript, Lang::JavaScript),
    ] {
        assert_eq!(
            syntax::outline(source, lang, 0..source.len(), 0, 0),
            [syntax::Segment::Kept {
                start: 0,
                end: source.len()
            }],
            "{lang:?}"
        );
    }
    // Non-signature lines between prototypes still elide in runs of 4.
    let gaps = "class Api {\npublic:\n    int a();\n    // one\n    // two\n    // three\n    // four\n    int b();\n};\n";
    assert_eq!(
        syntax::render_outline(gaps, &syntax::outline(gaps, Lang::Cpp, 0..gaps.len(), 0, 0)),
        "class Api {\npublic:\n    int a();\n    ⋯ 4-7\n    int b();\n};\n"
    );
    // A templated prototype keeps its multi-line template parameter list;
    // the non-signature lines before it (`public:` and four comments) fold.
    let templated = "class Api {\npublic:\n    // one\n    // two\n    // three\n    // four\n    template <\n        typename T,\n        typename U\n    >\n    T make(T, U);\n};\n";
    assert_eq!(
        syntax::render_outline(
            templated,
            &syntax::outline(templated, Lang::Cpp, 0..templated.len(), 0, 0)
        ),
        "class Api {\n⋯ 2-6\n    template <\n        typename T,\n        typename U\n    >\n    T make(T, U);\n};\n"
    );
}

#[test]
fn a_python_unit_range_ending_before_its_terminator_folds_its_interior() {
    // A Python function's range ends at its last statement, before the line
    // terminator; its 5-line interior (2-6) still folds in the signature form.
    let lines = [
        "def compute():",
        "    a = 1",
        "    b = 2",
        "    c = 3",
        "    d = 4",
        "    return a + b + c + d",
    ];
    for newline in ["\n", "\r\n"] {
        for final_newline in [true, false] {
            let mut source = lines.join(newline);
            if final_newline {
                source.push_str(newline);
            }
            let unit = syntax::units(&source, Lang::Python).remove(0);
            assert_eq!(&source[unit.start..unit.end], lines.join(newline));
            let segments = syntax::outline(&source, Lang::Python, unit.start..unit.end, 0, 0);
            assert_eq!(
                syntax::render_outline(&source, &segments),
                format!("def compute():{newline}    ⋯ 2-6\n"),
                "{newline:?} final newline {final_newline}"
            );
        }
    }
}

/// The declaration start of the unit whose range text begins at `text`.
fn decl_of(source: &str, lang: Lang, text: &str) -> usize {
    let start = source.find(text).unwrap();
    syntax::units(source, lang)
        .into_iter()
        .find(|unit| unit.start == start)
        .unwrap_or_else(|| panic!("no unit starts at {text:?}"))
        .decl
}

/// Leading run (context-v2 § Unit forest, 2026-10-04 amendment): Rust outer
/// doc comments and attributes join the unit; inner docs, plain comments, a
/// blank line and a comment that does not start its line stop the run.
#[test]
fn rust_outer_docs_and_attributes_join_their_unit() {
    let source = "\
//! Module docs stay outside.
use std::fmt;

/// The point.
/// Two lines.
#[derive(Debug)]
pub struct Point {
    x: u8,
}

/// Detached by a blank line.

fn detached() {}
fn trailing() {} /// A same-line comment.
fn after_trailing() {}
// A plain comment.
fn plain() {}
#[inline]
/// Docs after the attribute.
fn attribute_first() {}
";
    assert_eq!(
        units(source, Lang::Rust),
        [
            named("struct", "Point", span(source, "/// The point.", "\n}")),
            named("fn", "detached", "fn detached() {}"),
            named("fn", "trailing", "fn trailing() {}"),
            named("fn", "after_trailing", "fn after_trailing() {}"),
            named("fn", "plain", "fn plain() {}"),
            named(
                "fn",
                "attribute_first",
                span(source, "#[inline]", "fn attribute_first() {}")
            ),
        ]
    );
    assert_eq!(
        decl_of(source, Lang::Rust, "/// The point."),
        source.find("#[derive(Debug)]").unwrap(),
        "the first attribute starts the declaration"
    );
    assert_eq!(
        decl_of(source, Lang::Rust, "#[inline]"),
        source.find("#[inline]").unwrap()
    );
    assert_eq!(
        decl_of(source, Lang::Rust, "fn plain()"),
        source.find("fn plain()").unwrap(),
        "without a run the declaration starts at the node"
    );
    assert_tiles(source, Some(Lang::Rust));
}

/// Javadoc, JSDoc and Go doc comments join their unit; other comments, a
/// non-doc block comment and Python/C comments do not.
#[test]
fn javadoc_jsdoc_and_go_comments_join_their_unit_but_c_and_python_do_not() {
    let java = "\
/** Javadoc for A. */
public class A {
    // Plain.
    void plain() {}
    /**
     * Javadoc for m.
     */
    @Override
    public String m() { return \"\"; }
}
";
    assert_eq!(
        units(java, Lang::Java),
        [
            named("class", "A", java.trim_end()),
            named("method", "A.plain", "void plain() {}"),
            named(
                "method",
                "A.m",
                span(java, "/**\n     * Javadoc for m.", "return \"\"; }")
            ),
        ]
    );
    assert_eq!(
        decl_of(java, Lang::Java, "/**\n     * Javadoc for m."),
        java.find("@Override").unwrap()
    );

    let ts = "\
/** Adds. */
export function add(a: number, b: number) {
  return a + b;
}
// Not a doc.
function sub() {}
/* Not JSDoc. */
function mul() {}
";
    for lang in [Lang::TypeScript, Lang::JavaScript] {
        let source = if lang == Lang::JavaScript {
            ts.replace("a: number, b: number", "a, b")
        } else {
            ts.to_owned()
        };
        assert_eq!(
            units(&source, lang),
            [
                named("fn", "add", span(&source, "/** Adds. */", "\n}")),
                named("fn", "sub", "function sub() {}"),
                named("fn", "mul", "function mul() {}"),
            ],
            "{lang:?}"
        );
    }

    let go = "\
package p

// Add adds.
// Second line.
func Add(a, b int) int {
\treturn a + b
}

// Detached.

func Sub() {}
";
    assert_eq!(
        units(go, Lang::Go),
        [
            named("fn", "Add", span(go, "// Add adds.", "\n}")),
            named("fn", "Sub", "func Sub() {}"),
        ]
    );

    let python = "# A comment.\ndef f():\n    pass\n";
    assert_eq!(
        units(python, Lang::Python),
        [named("fn", "f", "def f():\n    pass")]
    );
    let c = "/** Doc. */\nint f(void) { return 0; }\n";
    assert_eq!(
        units(c, Lang::C),
        [named("fn", "f", "int f(void) { return 0; }")]
    );
}

/// A leading documentation run of at least 2 lines folds in the signature
/// form; the attribute that starts the declaration stays visible; a single
/// doc line is kept as text.
#[test]
fn leading_documentation_of_two_lines_folds_in_the_signature_form() {
    let source = "\
/// First.
/// Second.
/// Third.
#[inline]
pub fn documented() {
    let a = 1;
    let b = 2;
    let c = 3;
    let d = 4;
}
/// Only.
fn one() {
    let a = 1;
    let b = 2;
    let c = 3;
    let d = 4;
}
";
    let unit = |text: &str| {
        syntax::units(source, Lang::Rust)
            .into_iter()
            .find(|unit| source[unit.start..].starts_with(text))
            .unwrap()
    };
    let documented = unit("/// First.");
    let segments = syntax::outline(source, Lang::Rust, documented.start..documented.end, 0, 0);
    assert_eq!(
        syntax::render_outline(source, &segments),
        "⋯ 1-3\n#[inline]\npub fn documented() {\n    ⋯ 6-9\n}"
    );
    let one = unit("/// Only.");
    let segments = syntax::outline(source, Lang::Rust, one.start..one.end, 0, 0);
    assert_eq!(
        syntax::render_outline(source, &segments),
        "/// Only.\nfn one() {\n    ⋯ 13-16\n}"
    );
}

/// Leading-run boundaries: any Unicode whitespace may indent a run node or
/// fill a gap (NBSP and form feed are JavaScript whitespace), an empty `/**/`
/// still begins `/**`, and Rust inner docs or inner attributes directly above
/// an item never attach. `start`, `decl` and `head` are the run's start, the
/// first attribute and the item's own start.
#[test]
fn leading_run_boundaries_whitespace_empty_jsdoc_and_inner_rust_items() {
    for source in [
        "/** Quasar routing. */\u{a0}\nexport function route() {}\n",
        "\u{c}/** Quasar routing. */\nexport function route() {}\n",
    ] {
        assert_eq!(
            units(source, Lang::JavaScript),
            [named(
                "fn",
                "route",
                span(source, "/** Quasar", "route() {}")
            )],
            "{source:?}"
        );
        assert_tiles(source, Some(Lang::JavaScript));
    }

    let js = "/** Quasar routing. */\n/**/\nexport function route() {}\n";
    for lang in [Lang::JavaScript, Lang::TypeScript, Lang::Tsx] {
        assert_eq!(
            units(js, lang),
            [named("fn", "route", span(js, "/** Quasar", "route() {}"))],
            "{lang:?}"
        );
    }
    let java = "/** Doc for A. */\n/**/\nclass A {}\n";
    assert_eq!(
        units(java, Lang::Java),
        [named("class", "A", java.trim_end())]
    );

    for source in [
        "//! Inner docs.\nfn f() {}\n",
        "#![allow(dead_code)]\nfn f() {}\n",
    ] {
        let unit = syntax::units(source, Lang::Rust).remove(0);
        let at = source.find("fn f").unwrap();
        assert_eq!(
            (unit.start, unit.decl, unit.head),
            (at, at, at),
            "{source:?}"
        );
        assert_tiles(source, Some(Lang::Rust));
    }

    let source = "/// Doc.\n#[inline]\nfn g() {}\n";
    let unit = syntax::units(source, Lang::Rust).remove(0);
    assert_eq!(
        (unit.start, unit.decl, unit.head),
        (
            0,
            source.find("#[inline]").unwrap(),
            source.find("fn g").unwrap()
        )
    );
}

// --- 001 T007: name nodes, addresses and import keys (context-v2 § City map)

/// Each definition records its name node's range; a Rust `impl` extends a
/// type defined elsewhere, so it records none (and is no definition), while
/// its members' qualified names keep the type.
#[test]
fn definitions_record_their_name_node_and_impls_none() {
    let source = "pub struct Foo;\n\nimpl<T> Trait<T> for Foo {\n    fn bar(&self) {}\n}\n";
    let named: Vec<(&str, Option<&str>, Option<&str>)> = syntax::units(source, Lang::Rust)
        .iter()
        .map(|unit| {
            (
                unit.kind.as_str(),
                unit.qname.as_deref().map(|_| ""),
                unit.name_range.map(|(start, end)| &source[start..end]),
            )
        })
        .collect();
    assert_eq!(
        named,
        [
            ("struct", Some(""), Some("Foo")),
            ("impl", Some(""), None),
            ("fn", Some(""), Some("bar")),
        ]
    );
    for (lang, source, name) in [
        (Lang::Cpp, "void Box::grow(int n) {}\n", "grow"),
        (Lang::TypeScript, "export const run = () => {};\n", "run"),
        (
            Lang::Python,
            "@cached\ndef area(r):\n    return r\n",
            "area",
        ),
        (
            Lang::Go,
            "func (s *S) Close() error { return nil }\n",
            "Close",
        ),
    ] {
        let unit = syntax::units(source, lang).remove(0);
        let (start, end) = unit.name_range.expect("a definition");
        assert_eq!(&source[start..end], name, "{source:?}");
    }
    let section = syntax::units("# Title\n\ntext\n", Lang::Markdown).remove(0);
    assert_eq!(section.name_range, None, "a section is no definition");
}

/// The qualifiers of the unit named `qname` in `source`.
fn qualifiers(source: &str, lang: Lang, qname: &str) -> Vec<String> {
    let found = syntax::units(source, lang);
    found
        .iter()
        .find(|unit| unit.qname.as_deref() == Some(qname))
        .unwrap_or_else(|| panic!("no unit {qname} in {found:#?}"))
        .qualifiers
        .clone()
}

/// Address segments are the path's, then the qualified name's without the
/// unit's own name, read from the syntax tree: a generic type or template
/// contributes its base name and none of its arguments, whatever they hold
/// (`->`, a comment holding `>`, a `<<` shift, nested lists, lifetimes); a
/// scoped name contributes each part; the qualified name keeps the full text.
#[test]
fn address_segments_split_the_path_and_the_qualified_name() {
    let path = syntax::path_segments("packages/coding-agent/src/tools/index.ts");
    assert_eq!(
        path,
        ["packages", "coding", "agent", "src", "tools", "index"]
    );
    assert_eq!(syntax::path_segments(".gitignore"), ["gitignore"]);
    let nested =
        "mod graph {\n    impl Outer<Vec<[u8; 4]>> {\n        fn edges(&self) {}\n    }\n}\n";
    assert_eq!(
        syntax::address_segments(
            &syntax::path_segments("src/graph/mod.rs"),
            &qualifiers(nested, Lang::Rust, "graph::Outer<Vec<[u8; 4]>>::edges"),
        ),
        ["src", "graph", "mod", "outer"]
    );
    for (lang, source, qname, want) in [
        (
            Lang::Rust,
            "impl<Key> UnionFind<Key> {\n    fn find(&self) {}\n}\n",
            "UnionFind<Key>::find",
            &["unionfind"][..],
        ),
        (
            Lang::Rust,
            "impl Mapper<fn() -> u8> {\n    fn run(&self) {}\n}\n",
            "Mapper<fn() -> u8>::run",
            &["mapper"],
        ),
        (
            Lang::Rust,
            "impl Mapper</* > */ u8> {\n    fn run(&self) {}\n}\n",
            "Mapper</* > */ u8>::run",
            &["mapper"],
        ),
        (
            Lang::Rust,
            "impl Mapper<fn(Vec<u8>) -> Option<Box<[u8]>>> {\n    fn run(&self) {}\n}\n",
            "Mapper<fn(Vec<u8>) -> Option<Box<[u8]>>>::run",
            &["mapper"],
        ),
        (
            Lang::Rust,
            "impl<'a> Ref<'a, Slot<'a>> {\n    fn get(&self) {}\n}\n",
            "Ref<'a, Slot<'a>>::get",
            &["ref"],
        ),
        (
            Lang::Rust,
            "impl<T> a::b::Wrapper<T> {\n    fn get(&self) {}\n}\n",
            "a::b::Wrapper<T>::get",
            &["a", "b", "wrapper"],
        ),
        (
            Lang::Rust,
            "impl Show for &Bar {\n    fn show(&self) {}\n}\n",
            "&Bar::show",
            &["bar"],
        ),
        (
            Lang::Cpp,
            "template<> struct Box<1 << 2> {\n    void run() {}\n};\n",
            "Box<1 << 2>::run",
            &["box"],
        ),
        (
            Lang::Cpp,
            "struct ns::Box {\n    void run() {}\n};\n",
            "ns::Box::run",
            &["ns", "box"],
        ),
        (
            Lang::Cpp,
            "namespace a::b {\nvoid f() {}\n}\n",
            "a::b::f",
            &["a", "b"],
        ),
        (
            Lang::Java,
            "class Outer {\n  class Inner {\n    void run() {}\n  }\n}\n",
            "Outer.Inner.run",
            &["outer", "inner"],
        ),
    ] {
        assert_eq!(qualifiers(source, lang, qname), want, "{qname}");
    }
    // The scope of a unit's own name qualifies it.
    assert_eq!(
        qualifiers(
            "struct ns::Box {\n    void run() {}\n};\n",
            Lang::Cpp,
            "ns::Box"
        ),
        ["ns"]
    );
}

/// Each file's import keys (context-v2 § Doors import keys): the bound names
/// an import introduces, a path's last segment, an include's or require's
/// file stem; glob imports give none.
#[test]
fn import_keys_name_what_each_import_binds() {
    for (lang, source, keys) in [
        (
            Lang::Rust,
            "use crate::store::Engine;\nuse std::io::{self, Read as R, prelude::*};\nuse a::b::*;\nuse helper;\nfn f() {}\n",
            vec!["Engine", "io", "R", "helper"],
        ),
        (
            Lang::Python,
            "import os.path\nimport numpy as np\nfrom m import a, b as c\nfrom . import d\nfrom x import *\n",
            vec!["path", "np", "a", "c", "d"],
        ),
        (
            Lang::TypeScript,
            "import Def, { A, B as C } from './m';\nimport * as ns from 'pkg';\nimport type { T } from './t';\nimport './side';\nimport x = require('./legacy');\n",
            vec!["Def", "A", "C", "ns", "T", "x"],
        ),
        (
            Lang::JavaScript,
            "const tools = require('./tools/index.js');\nimport { run } from \"./run\";\n",
            vec!["index", "run"],
        ),
        (
            Lang::Go,
            "package p\n\nimport (\n\t\"fmt\"\n\tf \"github.com/a/flags\"\n\t. \"strings\"\n\t_ \"embed\"\n)\n",
            vec!["fmt", "f"],
        ),
        (
            Lang::C,
            "#include \"x/y.h\"\n#include <sys/types.h>\nint main(void) { return 0; }\n",
            vec!["y", "types"],
        ),
        (
            Lang::Cpp,
            "#include <vector>\nusing namespace std;\nusing ns::Widget;\nint f() { return 0; }\n",
            vec!["vector", "std", "Widget"],
        ),
        (
            Lang::Java,
            "import java.util.List;\nimport static org.junit.Assert.assertEquals;\nimport java.io.*;\nclass A {}\n",
            vec!["List", "assertEquals"],
        ),
    ] {
        assert_eq!(
            syntax::index(source, Some(lang)).unwrap().imports,
            keys,
            "{lang:?}"
        );
    }
    // Keys are distinct; documents are the plain document tiling.
    let source = "use a::X;\nuse b::X;\nfn f() {}\n";
    let index = syntax::index(source, Some(Lang::Rust)).unwrap();
    assert_eq!(index.imports, ["X"]);
    assert_eq!(index.documents, syntax::documents(source, Some(Lang::Rust)));
    assert!(syntax::index(source, None).unwrap().imports.is_empty());
}

/// Expected units of one source: `(kind, qualified name, name text)`.
type ExpectedUnits = &'static [(&'static str, &'static str, &'static str)];

/// Every definition gets an address (context-v2 § Unit kinds, amended for
/// 001 T007): enum members, module-level bindings and Go specs are units
/// with their name node and qualified name; fields and local bindings are
/// not.
#[test]
fn enum_members_and_module_level_bindings_are_definitions() {
    let cases: [(Lang, &str, ExpectedUnits); 7] = [
        (
            Lang::Rust,
            "enum Color {\n    Red,\n    Green(u8),\n}\nstruct P {\n    field: u8,\n}\nfn f() {\n    let local = 1;\n}\n",
            &[
                ("variant", "Color::Red", "Red"),
                ("variant", "Color::Green", "Green"),
            ],
        ),
        (
            Lang::Python,
            "LIMIT = 10\nclass A:\n    field = 1\ndef f():\n    local = 2\n",
            &[("static", "LIMIT", "LIMIT")],
        ),
        (
            Lang::TypeScript,
            "export const LIMIT = 10;\nlet counter = 0, other = 1;\nconst run = () => {};\nenum Mode { Fast, Slow = 2 }\nclass K {\n  field = 1;\n}\nfunction g() {\n  const local = 1;\n}\n",
            &[
                ("const", "LIMIT", "LIMIT"),
                ("static", "counter", "counter"),
                ("static", "other", "other"),
                ("fn", "run", "run"),
                ("variant", "Mode.Fast", "Fast"),
                ("variant", "Mode.Slow", "Slow"),
            ],
        ),
        (
            Lang::Go,
            "package p\n\ntype Size int\n\ntype Alias = Size\n\nconst Max = 3\n\nvar count int\n\nfunc f() {\n\tvar local = 1\n\t_ = local\n}\n",
            &[
                ("type", "Size", "Size"),
                ("type", "Alias", "Alias"),
                ("const", "Max", "Max"),
                ("static", "count", "count"),
            ],
        ),
        (
            Lang::C,
            "enum color { RED, GREEN = 2 };\nstruct s {\n  int field;\n};\n",
            &[
                ("variant", "color.RED", "RED"),
                ("variant", "color.GREEN", "GREEN"),
            ],
        ),
        (
            Lang::Cpp,
            "enum class Mode { Fast };\n",
            &[("variant", "Mode::Fast", "Fast")],
        ),
        (
            Lang::Java,
            "enum Mode { FAST, SLOW; }\nclass K {\n  int field;\n}\n",
            &[
                ("variant", "Mode.FAST", "FAST"),
                ("variant", "Mode.SLOW", "SLOW"),
            ],
        ),
    ];
    for (lang, source, want) in cases {
        let found = syntax::units(source, lang);
        for &(kind, qname, name) in want {
            let unit = found
                .iter()
                .find(|unit| unit.qname.as_deref() == Some(qname))
                .unwrap_or_else(|| panic!("{lang:?}: no unit {qname} in {found:#?}"));
            assert_eq!(unit.kind.as_str(), kind, "{lang:?} {qname}");
            let (start, end) = unit.name_range.expect("a definition");
            assert_eq!(&source[start..end], name, "{lang:?} {qname}");
        }
        for unit in &found {
            let name = unit.name.as_deref().unwrap_or("");
            assert!(
                !matches!(name, "field" | "local"),
                "{lang:?}: a field or local binding became a unit: {unit:?}"
            );
        }
        assert_tiles(source, Some(lang));
    }
    // The single-declarator declaration (with its `export`) is the range.
    let source = "export const LIMIT = 10;\n";
    let unit = syntax::units(source, Lang::TypeScript).remove(0);
    assert_eq!(&source[unit.start..unit.end], "export const LIMIT = 10;");
    // An enum's outline still shows its variant lines.
    let source = "enum Color {\n    Red,\n    Green,\n    Blue,\n    Cyan,\n    Magenta,\n}\n";
    let shown = syntax::render_outline(
        source,
        &syntax::outline(source, Lang::Rust, 0..source.len(), 0, 0),
    );
    assert_eq!(shown, source);
}

/// A quoted TypeScript enum member is a variant like a bare one (the enum
/// body's `name` field holds either), with or without an initializer, and a
/// quoted method name is a name like any other: the name is the source text
/// inside the quotes, as written — escapes are kept, not decoded.
#[test]
fn quoted_typescript_enum_members_are_variants_named_inside_their_quotes() {
    let source = concat!(
        "enum Mode { \"Fast\", Slow = 2, \"Quick\" = 3, \"F\\u0061r\", 'Q\\'t' = 4 }\n",
        "class Api {\n  \"get\\u0056alue\"() {}\n}\n",
    );
    let found = syntax::units(source, Lang::TypeScript);
    let shown: Vec<(&str, Option<&str>, &str, &str)> = found
        .iter()
        .map(|unit| {
            let (start, end) = unit.name_range.expect("a definition");
            (
                unit.kind.as_str(),
                unit.qname.as_deref(),
                &source[unit.start..unit.end],
                &source[start..end],
            )
        })
        .collect();
    let class = &source[source.find("class").unwrap()..source.len() - 1];
    assert_eq!(
        shown,
        [
            ("enum", Some("Mode"), source.lines().next().unwrap(), "Mode"),
            ("variant", Some("Mode.Fast"), "\"Fast\"", "Fast"),
            ("variant", Some("Mode.Slow"), "Slow = 2", "Slow"),
            ("variant", Some("Mode.Quick"), "\"Quick\" = 3", "Quick"),
            (
                "variant",
                Some("Mode.F\\u0061r"),
                "\"F\\u0061r\"",
                "F\\u0061r"
            ),
            ("variant", Some("Mode.Q\\'t"), "'Q\\'t' = 4", "Q\\'t"),
            ("class", Some("Api"), class, "Api"),
            (
                "method",
                Some("Api.get\\u0056alue"),
                "\"get\\u0056alue\"() {}",
                "get\\u0056alue"
            ),
        ]
    );
    // One document holds each name: one definition each.
    let documents = assert_tiles(source, Some(Lang::TypeScript));
    for unit in &found {
        let (name_start, _) = unit.name_range.unwrap();
        let holding = documents
            .iter()
            .filter(|document| document.start <= name_start && name_start < document.end)
            .filter(|document| document.unit.qname == unit.qname)
            .count();
        assert_eq!(holding, 1, "{:?}", unit.qname);
    }
}

/// A Go declaration with one spec supplies the range and the leading docs,
/// also when grouped: the grammar's `var_spec_list` sits between `var ( … )`
/// and its spec. With several specs each spec keeps its own range and docs.
#[test]
fn a_grouped_go_declaration_with_one_spec_is_the_range() {
    let source = "package p\n\n// Count is documented.\nvar (\n\tcount int\n)\n\nvar (\n\t// First is documented.\n\tfirst int\n\tsecond int\n)\n\n// Limit is documented.\nconst (\n\tLimit = 3\n)\n";
    assert_eq!(
        units(source, Lang::Go),
        [
            named(
                "static",
                "count",
                "// Count is documented.\nvar (\n\tcount int\n)"
            ),
            named("static", "first", "// First is documented.\n\tfirst int"),
            named("static", "second", "second int"),
            named(
                "const",
                "Limit",
                "// Limit is documented.\nconst (\n\tLimit = 3\n)"
            ),
        ]
    );
    for unit in syntax::units(source, Lang::Go) {
        let (start, end) = unit.name_range.expect("a definition");
        assert_eq!(Some(&source[start..end]), unit.name.as_deref());
    }
    assert_tiles(source, Some(Lang::Go));
}

/// Perl's last block runs over the file's trailing whitespace and comments;
/// its package statement still contains it, so the sub keeps its unit.
#[test]
fn a_perl_package_keeps_a_last_sub_whose_block_runs_to_the_end() {
    for tail in ["\n", "\n\n# trailing comment\n", ""] {
        let source = format!("package A::B;\nsub f {{\n  return 1;\n}}{tail}");
        let found = units(&source, Lang::Perl);
        let kinds: Vec<_> = found
            .iter()
            .map(|(kind, qname, _)| (*kind, qname.as_deref()))
            .collect();
        assert_eq!(
            kinds,
            [("mod", Some("A::B")), ("fn", Some("A::B::f"))],
            "{tail:?}"
        );
        assert!(found[1].2.starts_with("sub f {"), "{tail:?}");
        assert_tiles(&source, Some(Lang::Perl));
    }
}

// --- 001 T008: languages beyond the first eight (context-v2 § City map ›
// Languages)

/// Per unit, in source order: `(kind, qualified name, name-node text, first
/// line of its range, last line of its range)`; `""` for none.
type Rows = &'static [(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
)];

/// One fixture per new language (`tests/fixtures/syntax`): nested
/// containers, an overloaded name or a name at two scopes, a method in a
/// nested type where the language has nested types, an attributed or
/// annotated definition, a `?`/`!`/`'` suffix where the language allows one,
/// and one import of each kind. Fields, locals, interface and protocol
/// members, signatures and bodiless protocol heads are no units.
const FIXTURES: &[(&str, &str, Rows, &[&str])] = &[
    (
        "store.cs",
        include_str!("fixtures/syntax/store.cs"),
        &[
            ("mod", "Outer.Space", "Space", "namespace Outer.Space", "}"),
            (
                "mod",
                "Outer.Space.Inner",
                "Inner",
                "namespace Inner",
                "    }",
            ),
            (
                "class",
                "Outer.Space.Inner.Store",
                "Store",
                "[Serializable]",
                "        }",
            ),
            (
                "class",
                "Outer.Space.Inner.Store.Nested",
                "Nested",
                "public class Nested",
                "            }",
            ),
            (
                "method",
                "Outer.Space.Inner.Store.Nested.Put",
                "Put",
                "[Obsolete(\"x\")]",
                "                public void Put(int a) {}",
            ),
            (
                "method",
                "Outer.Space.Inner.Store.Nested.Put",
                "Put",
                "public void Put(string a) {}",
                "public void Put(string a) {}",
            ),
            (
                "method",
                "Outer.Space.Inner.Store.Store",
                "Store",
                "public Store() {}",
                "public Store() {}",
            ),
            (
                "method",
                "Outer.Space.Inner.Store.Native",
                "Native",
                "static extern int Native(int x);",
                "static extern int Native(int x);",
            ),
            (
                "interface",
                "Outer.Space.Inner.IShape",
                "IShape",
                "public interface IShape { double Area(); }",
                "public interface IShape { double Area(); }",
            ),
            (
                "struct",
                "Outer.Space.Inner.Point",
                "Point",
                "public struct Point { public int X; }",
                "public struct Point { public int X; }",
            ),
            (
                "enum",
                "Outer.Space.Inner.Color",
                "Color",
                "public enum Color { Red, Green = 2 }",
                "public enum Color { Red, Green = 2 }",
            ),
            (
                "variant",
                "Outer.Space.Inner.Color.Red",
                "Red",
                "Red",
                "Red",
            ),
            (
                "variant",
                "Outer.Space.Inner.Color.Green",
                "Green",
                "Green = 2",
                "Green = 2",
            ),
            (
                "class",
                "Outer.Space.Inner.Pair",
                "Pair",
                "public record Pair(int A, int B);",
                "public record Pair(int A, int B);",
            ),
            (
                "type",
                "Outer.Space.Inner.Handler",
                "Handler",
                "public delegate void Handler(object s);",
                "public delegate void Handler(object s);",
            ),
        ],
        &["Generic", "Math", "IO", "Linq"],
    ),
    (
        "flat.cs",
        include_str!("fixtures/syntax/flat.cs"),
        &[
            ("mod", "Outer.Flat", "Flat", "namespace Outer.Flat;", "}"),
            ("class", "Outer.Flat.A", "A", "class A", "}"),
            ("method", "Outer.Flat.A.Run", "Run", "void Run()", "    }"),
            (
                "fn",
                "Outer.Flat.A.Run.Local",
                "Local",
                "int Local() => 1;",
                "int Local() => 1;",
            ),
        ],
        &[],
    ),
    (
        "shapes.fs",
        include_str!("fixtures/syntax/shapes.fs"),
        &[
            (
                "mod",
                "Outer.Space",
                "Space",
                "namespace Outer.Space",
                "    exception Failure of string",
            ),
            (
                "mod",
                "Outer.Space.Inner",
                "Inner",
                "module Inner =",
                "    exception Failure of string",
            ),
            (
                "const",
                "Outer.Space.Inner.Limit",
                "Limit",
                "[<Literal>]",
                "    let Limit = 10",
            ),
            (
                "static",
                "Outer.Space.Inner.counter",
                "counter",
                "let mutable counter = 0",
                "let mutable counter = 0",
            ),
            (
                "fn",
                "Outer.Space.Inner.area",
                "area'",
                "let area' r = r * r",
                "let area' r = r * r",
            ),
            (
                "enum",
                "Outer.Space.Inner.Shape",
                "Shape",
                "type Shape =",
                "        | Square of float",
            ),
            (
                "variant",
                "Outer.Space.Inner.Shape.Circle",
                "Circle",
                "Circle of float",
                "Circle of float",
            ),
            (
                "variant",
                "Outer.Space.Inner.Shape.Square",
                "Square",
                "Square of float",
                "Square of float",
            ),
            (
                "class",
                "Outer.Space.Inner.Store",
                "Store",
                "type Store() =",
                "        default this.Size() = 0",
            ),
            (
                "method",
                "Outer.Space.Inner.Store.Put",
                "Put",
                "member this.Put(a: int) = ()",
                "member this.Put(a: int) = ()",
            ),
            (
                "method",
                "Outer.Space.Inner.Store.Put",
                "Put",
                "member this.Put(a: string) = ()",
                "member this.Put(a: string) = ()",
            ),
            (
                "method",
                "Outer.Space.Inner.Store.Size",
                "Size",
                "default this.Size() = 0",
                "default this.Size() = 0",
            ),
            (
                "enum",
                "Outer.Space.Inner.Color",
                "Color",
                "type Color =",
                "        | Green = 1",
            ),
            (
                "variant",
                "Outer.Space.Inner.Color.Red",
                "Red",
                "Red = 0",
                "Red = 0",
            ),
            (
                "variant",
                "Outer.Space.Inner.Color.Green",
                "Green",
                "Green = 1",
                "Green = 1",
            ),
            (
                "class",
                "Outer.Space.Inner.Point",
                "Point",
                "type Point = { X: int; Y: int }",
                "type Point = { X: int; Y: int }",
            ),
            (
                "impl",
                "Outer.Space.Inner.System.String",
                "",
                "type System.String with",
                "        member x.Shout() = x.ToUpper()",
            ),
            (
                "method",
                "Outer.Space.Inner.System.String.Shout",
                "Shout",
                "member x.Shout() = x.ToUpper()",
                "member x.Shout() = x.ToUpper()",
            ),
            (
                "mod",
                "Outer.Space.Inner.Nested",
                "Nested",
                "module Nested =",
                "            member this.Run() = 1",
            ),
            (
                "class",
                "Outer.Space.Inner.Nested.Deep",
                "Deep",
                "type Deep() =",
                "            member this.Run() = 1",
            ),
            (
                "method",
                "Outer.Space.Inner.Nested.Deep.Run",
                "Run",
                "member this.Run() = 1",
                "member this.Run() = 1",
            ),
            (
                "fn",
                "Outer.Space.Inner.outer",
                "outer",
                "let outer x =",
                "        inner x + local",
            ),
            (
                "fn",
                "Outer.Space.Inner.outer.inner",
                "inner",
                "let inner y = y + 1",
                "let inner y = y + 1",
            ),
            (
                "type",
                "Outer.Space.Inner.Failure",
                "Failure",
                "exception Failure of string",
                "exception Failure of string",
            ),
        ],
        &["Generic", "Math", "tools"],
    ),
    (
        "shapes.fsi",
        include_str!("fixtures/syntax/shapes.fsi"),
        &[
            (
                "mod",
                "Outer.Space",
                "Space",
                "namespace Outer.Space",
                "    type Point = { X: int; Y: int }",
            ),
            (
                "mod",
                "Outer.Space.Inner",
                "Inner",
                "module Inner =",
                "    type Point = { X: int; Y: int }",
            ),
            (
                "class",
                "Outer.Space.Inner.Point",
                "Point",
                "type Point = { X: int; Y: int }",
                "type Point = { X: int; Y: int }",
            ),
        ],
        &[],
    ),
    (
        "shapes.vb",
        include_str!("fixtures/syntax/shapes.vb"),
        &[
            (
                "mod",
                "Outer.Space",
                "Space",
                "Namespace Outer.Space",
                "End Namespace",
            ),
            (
                "mod",
                "Outer.Space.Inner",
                "Inner",
                "Namespace Inner",
                "    End Namespace",
            ),
            (
                "class",
                "Outer.Space.Inner.Store",
                "Store",
                "<Serializable>",
                "        End Class",
            ),
            (
                "method",
                "Outer.Space.Inner.Store.New",
                "New",
                "Public Sub New()",
                "            End Sub",
            ),
            (
                "method",
                "Outer.Space.Inner.Store.Put",
                "Put",
                "<Obsolete(\"x\")>",
                "            End Sub",
            ),
            (
                "method",
                "Outer.Space.Inner.Store.Put",
                "Put",
                "Public Sub Put(a As String)",
                "            End Sub",
            ),
            (
                "method",
                "Outer.Space.Inner.Store.Size",
                "Size",
                "Public Function Size() As Integer",
                "            End Function",
            ),
            (
                "interface",
                "Outer.Space.Inner.IShape",
                "IShape",
                "Public Interface IShape",
                "        End Interface",
            ),
            (
                "struct",
                "Outer.Space.Inner.Point",
                "Point",
                "Public Structure Point",
                "        End Structure",
            ),
            (
                "enum",
                "Outer.Space.Inner.Color",
                "Color",
                "Public Enum Color",
                "        End Enum",
            ),
            (
                "variant",
                "Outer.Space.Inner.Color.Red",
                "Red",
                "Red",
                "Red",
            ),
            (
                "variant",
                "Outer.Space.Inner.Color.Green",
                "Green",
                "Green = 2",
                "Green = 2",
            ),
            (
                "mod",
                "Outer.Space.Inner.Helpers",
                "Helpers",
                "Public Module Helpers",
                "        End Module",
            ),
            (
                "const",
                "Outer.Space.Inner.Helpers.Max",
                "Max",
                "Const Max As Integer = 3",
                "Const Max As Integer = 3",
            ),
            (
                "static",
                "Outer.Space.Inner.Helpers.total",
                "total",
                "Private total As Integer",
                "Private total As Integer",
            ),
            (
                "method",
                "Outer.Space.Inner.Helpers.Run",
                "Run",
                "Sub Run()",
                "            End Sub",
            ),
            (
                "type",
                "Outer.Space.Inner.Handler",
                "Handler",
                "Public Delegate Sub Handler(s As Object)",
                "Public Delegate Sub Handler(s As Object)",
            ),
        ],
        &["Generic", "Linq"],
    ),
    (
        "account.php",
        include_str!("fixtures/syntax/account.php"),
        &[
            (
                "mod",
                "App.Models",
                "Models",
                "namespace App\\Models;",
                "function make() {}",
            ),
            (
                "const",
                "App.Models.LIMIT",
                "LIMIT",
                "const LIMIT = 10;",
                "const LIMIT = 10;",
            ),
            ("class", "App.Models.Account", "Account", "#[Entity]", "}"),
            (
                "method",
                "App.Models.Account.make",
                "make",
                "public function make(int $a) { return $a; }",
                "public function make(int $a) { return $a; }",
            ),
            (
                "method",
                "App.Models.Account.create",
                "create",
                "public static function create() {",
                "    }",
            ),
            (
                "interface",
                "App.Models.Shape",
                "Shape",
                "interface Shape",
                "}",
            ),
            ("trait", "App.Models.Greets", "Greets", "trait Greets", "}"),
            (
                "method",
                "App.Models.Greets.hello",
                "hello",
                "public function hello() {}",
                "public function hello() {}",
            ),
            ("enum", "App.Models.Suit", "Suit", "enum Suit: string", "}"),
            (
                "variant",
                "App.Models.Suit.Hearts",
                "Hearts",
                "case Hearts = 'H';",
                "case Hearts = 'H';",
            ),
            (
                "variant",
                "App.Models.Suit.Spades",
                "Spades",
                "case Spades = 'S';",
                "case Spades = 'S';",
            ),
            (
                "fn",
                "App.Models.make",
                "make",
                "function make() {}",
                "function make() {}",
            ),
            (
                "mod",
                "App.Other",
                "Other",
                "namespace App\\Other;",
                "function other() {}",
            ),
            (
                "fn",
                "App.Other.other",
                "other",
                "function other() {}",
                "function other() {}",
            ),
        ],
        &["Bar", "Qux", "One", "Deux", "helper", "boot", "util"],
    ),
    (
        "braced.php",
        include_str!("fixtures/syntax/braced.php"),
        &[
            (
                "mod",
                "Braced.Space",
                "Space",
                "namespace Braced\\Space {",
                "}",
            ),
            (
                "class",
                "Braced.Space.Inner",
                "Inner",
                "class Inner {",
                "    }",
            ),
            (
                "method",
                "Braced.Space.Inner.run",
                "run",
                "public function run() {}",
                "public function run() {}",
            ),
        ],
        &[],
    ),
    (
        "shape.pl",
        include_str!("fixtures/syntax/shape.pl"),
        &[
            ("mod", "Outer::Shape", "Shape", "package Outer::Shape;", "}"),
            (
                "const",
                "Outer::Shape::LIMIT",
                "LIMIT",
                "use constant LIMIT => 10;",
                "use constant LIMIT => 10;",
            ),
            ("fn", "Outer::Shape::new", "new", "sub new {", "}"),
            (
                "fn",
                "Outer::Shape::area",
                "area",
                "sub area : lvalue {",
                "}",
            ),
            (
                "mod",
                "Outer::Other",
                "Other",
                "package Outer::Other {",
                "}",
            ),
            (
                "fn",
                "Outer::Other::run",
                "run",
                "sub run { 1 }",
                "sub run { 1 }",
            ),
            (
                "fn",
                "Outer::Other::helper",
                "helper",
                "sub helper {",
                "    }",
            ),
            (
                "mod",
                "Outer::Shape::Circle",
                "Circle",
                "package Outer::Shape::Circle;",
                "1;",
            ),
            (
                "fn",
                "Outer::Shape::Circle::area",
                "area",
                "sub area { 2 }",
                "sub area { 2 }",
            ),
        ],
        &["strict", "Util", "Thing", "Dumper"],
    ),
    (
        "tools.sh",
        include_str!("fixtures/syntax/tools.sh"),
        &[
            (
                "const",
                "LIMIT",
                "LIMIT",
                "readonly LIMIT=10",
                "readonly LIMIT=10",
            ),
            (
                "const",
                "MAX",
                "MAX",
                "declare -r MAX=3",
                "declare -r MAX=3",
            ),
            ("static", "COUNT", "COUNT", "COUNT=0", "COUNT=0"),
            (
                "static",
                "PATH_PREFIX",
                "PATH_PREFIX",
                "export PATH_PREFIX=/usr",
                "export PATH_PREFIX=/usr",
            ),
            ("fn", "outer", "outer", "outer() {", "}"),
            ("fn", "outer.inner", "inner", "inner() {", "    }"),
            ("fn", "with-dash", "with-dash", "function with-dash {", "}"),
            ("fn", "both", "both", "function both() {", "}"),
        ],
        &["common", "helpers"],
    ),
    (
        "tools.ps1",
        include_str!("fixtures/syntax/tools.ps1"),
        &[
            ("fn", "Get-Thing", "Get-Thing", "function Get-Thing {", "}"),
            (
                "fn",
                "Get-Thing.Inner-Helper",
                "Inner-Helper",
                "function Inner-Helper { 1 }",
                "function Inner-Helper { 1 }",
            ),
            ("class", "Store", "Store", "class Store {", "}"),
            ("method", "Store.Store", "Store", "Store() {}", "Store() {}"),
            (
                "method",
                "Store.Put",
                "Put",
                "[void] Put([int]$a) {}",
                "[void] Put([int]$a) {}",
            ),
            (
                "method",
                "Store.Put",
                "Put",
                "[void] Put([string]$a) {}",
                "[void] Put([string]$a) {}",
            ),
            ("enum", "Color", "Color", "enum Color {", "}"),
            ("variant", "Color.Red", "Red", "Red", "Red"),
            ("variant", "Color.Green", "Green", "Green = 2", "Green = 2"),
        ],
        &["Generic", "Helpers", "Accounts", "common"],
    ),
    (
        "store.rb",
        include_str!("fixtures/syntax/store.rb"),
        &[
            ("const", "LIMIT", "LIMIT", "LIMIT = 10", "LIMIT = 10"),
            ("mod", "Outer", "Outer", "module Outer", "end"),
            ("mod", "Outer::Inner", "Inner", "module Inner", "  end"),
            (
                "class",
                "Outer::Inner::Store",
                "Store",
                "class Store < Base",
                "    end",
            ),
            (
                "const",
                "Outer::Inner::Store::RATE",
                "RATE",
                "RATE = 2",
                "RATE = 2",
            ),
            (
                "method",
                "Outer::Inner::Store::put",
                "put",
                "def put(a)",
                "      end",
            ),
            (
                "method",
                "Outer::Inner::Store::empty",
                "empty?",
                "def empty?",
                "      end",
            ),
            (
                "method",
                "Outer::Inner::Store::save",
                "save!",
                "def save!",
                "      end",
            ),
            (
                "method",
                "Outer::Inner::Store::name",
                "name",
                "def name=(v)",
                "      end",
            ),
            (
                "method",
                "Outer::Inner::Store::create",
                "create",
                "def self.create",
                "      end",
            ),
            (
                "method",
                "Outer::Inner::Store::hidden",
                "hidden",
                "private def hidden",
                "      end",
            ),
            (
                "method",
                "Outer::Inner::Store::build",
                "build",
                "def build",
                "        end",
            ),
            (
                "class",
                "Outer::Inner::Store::Nested",
                "Nested",
                "class Nested",
                "      end",
            ),
            (
                "method",
                "Outer::Inner::Store::Nested::run",
                "run",
                "def run",
                "        end",
            ),
            (
                "class",
                "Outer::Inner::Store",
                "Store",
                "class Outer::Inner::Store",
                "end",
            ),
            (
                "method",
                "Outer::Inner::Store::put",
                "put",
                "def put(a, b)",
                "  end",
            ),
        ],
        &["json", "helpers", "setup", "core"],
    ),
    (
        "Store.kt",
        include_str!("fixtures/syntax/Store.kt"),
        &[
            (
                "const",
                "LIMIT",
                "LIMIT",
                "const val LIMIT = 10",
                "const val LIMIT = 10",
            ),
            (
                "const",
                "greeting",
                "greeting",
                "val greeting = \"hi\"",
                "val greeting = \"hi\"",
            ),
            (
                "static",
                "counter",
                "counter",
                "var counter = 0",
                "var counter = 0",
            ),
            ("class", "Store", "Store", "@Entity", "}"),
            (
                "method",
                "Store.constructor",
                "constructor",
                "constructor(x: Int) {}",
                "constructor(x: Int) {}",
            ),
            (
                "method",
                "Store.put",
                "put",
                "fun put(a: Int) {}",
                "fun put(a: Int) {}",
            ),
            (
                "method",
                "Store.put",
                "put",
                "fun put(a: String) {}",
                "fun put(a: String) {}",
            ),
            ("class", "Store.Nested", "Nested", "class Nested {", "    }"),
            (
                "method",
                "Store.Nested.run",
                "run",
                "fun run() {",
                "        }",
            ),
            (
                "method",
                "Store.create",
                "create",
                "fun create(): Store = Store(1)",
                "fun create(): Store = Store(1)",
            ),
            ("interface", "Shape", "Shape", "interface Shape {", "}"),
            (
                "method",
                "Shape.describe",
                "describe",
                "fun describe(): String = \"shape\"",
                "fun describe(): String = \"shape\"",
            ),
            ("enum", "Color", "Color", "enum class Color {", "}"),
            ("variant", "Color.RED", "RED", "RED", "RED"),
            ("variant", "Color.GREEN", "GREEN", "GREEN", "GREEN"),
            ("class", "Registry", "Registry", "object Registry {", "}"),
            (
                "method",
                "Registry.register",
                "register",
                "fun register() {}",
                "fun register() {}",
            ),
            (
                "type",
                "Name",
                "Name",
                "typealias Name = String",
                "typealias Name = String",
            ),
            (
                "fn",
                "Store.extension",
                "extension",
                "fun Store.extension() {}",
                "fun Store.extension() {}",
            ),
            (
                "class",
                "Point",
                "Point",
                "data class Point(val x: Int, val y: Int)",
                "data class Point(val x: Int, val y: Int)",
            ),
        ],
        &["PI", "R"],
    ),
    (
        "Store.swift",
        include_str!("fixtures/syntax/Store.swift"),
        &[
            (
                "const",
                "limit",
                "limit",
                "let limit = 10",
                "let limit = 10",
            ),
            (
                "static",
                "counter",
                "counter",
                "var counter = 0",
                "var counter = 0",
            ),
            ("class", "Store", "Store", "@MainActor", "}"),
            (
                "method",
                "Store.put",
                "put",
                "func put(_ a: Int) {}",
                "func put(_ a: Int) {}",
            ),
            (
                "method",
                "Store.put",
                "put",
                "func put(_ a: String) {}",
                "func put(_ a: String) {}",
            ),
            ("method", "Store.init", "init", "init() {}", "init() {}"),
            (
                "struct",
                "Store.Nested",
                "Nested",
                "struct Nested {",
                "    }",
            ),
            (
                "method",
                "Store.Nested.run",
                "run",
                "func run() {",
                "        }",
            ),
            ("interface", "Shape", "Shape", "protocol Shape {", "}"),
            ("enum", "Color", "Color", "enum Color {", "}"),
            ("variant", "Color.red", "red", "case red", "case red"),
            ("variant", "Color.green", "green", "green", "green"),
            ("variant", "Color.blue", "blue", "blue", "blue"),
            ("impl", "Store", "", "extension Store {", "}"),
            (
                "method",
                "Store.extra",
                "extra",
                "func extra() {}",
                "func extra() {}",
            ),
            ("impl", "Outer.Inner", "", "extension Outer.Inner {", "}"),
            (
                "method",
                "Outer.Inner.deep",
                "deep",
                "func deep() {}",
                "func deep() {}",
            ),
            (
                "type",
                "Name",
                "Name",
                "typealias Name = String",
                "typealias Name = String",
            ),
            (
                "fn",
                "helper",
                "helper",
                "func helper() -> Int { return 1 }",
                "func helper() -> Int { return 1 }",
            ),
        ],
        &["Foundation", "Array", "MyModule"],
    ),
    (
        "shapes.scala",
        include_str!("fixtures/syntax/shapes.scala"),
        &[
            (
                "mod",
                "com.example",
                "example",
                "package com.example",
                "def helper(): Int = 1",
            ),
            (
                "mod",
                "com.example.shapes",
                "shapes",
                "package shapes",
                "def helper(): Int = 1",
            ),
            (
                "const",
                "com.example.shapes.limit",
                "limit",
                "val limit = 10",
                "val limit = 10",
            ),
            (
                "static",
                "com.example.shapes.counter",
                "counter",
                "var counter = 0",
                "var counter = 0",
            ),
            (
                "class",
                "com.example.shapes.Store",
                "Store",
                "@deprecated(\"x\", \"1\")",
                "}",
            ),
            (
                "method",
                "com.example.shapes.Store.put",
                "put",
                "def put(a: Int): Unit = {}",
                "def put(a: Int): Unit = {}",
            ),
            (
                "method",
                "com.example.shapes.Store.put",
                "put",
                "def put(a: String): Unit = {}",
                "def put(a: String): Unit = {}",
            ),
            (
                "class",
                "com.example.shapes.Store.Nested",
                "Nested",
                "class Nested {",
                "  }",
            ),
            (
                "method",
                "com.example.shapes.Store.Nested.run",
                "run",
                "def run(): Unit = {",
                "    }",
            ),
            (
                "trait",
                "com.example.shapes.Shape",
                "Shape",
                "trait Shape {",
                "}",
            ),
            (
                "class",
                "com.example.shapes.Registry",
                "Registry",
                "object Registry {",
                "}",
            ),
            (
                "method",
                "com.example.shapes.Registry.register",
                "register",
                "def register(): Unit = ()",
                "def register(): Unit = ()",
            ),
            (
                "enum",
                "com.example.shapes.Color",
                "Color",
                "enum Color {",
                "}",
            ),
            (
                "variant",
                "com.example.shapes.Color.Red",
                "Red",
                "Red",
                "Red",
            ),
            (
                "variant",
                "com.example.shapes.Color.Green",
                "Green",
                "Green",
                "Green",
            ),
            (
                "class",
                "com.example.shapes.Point",
                "Point",
                "case class Point(x: Int, y: Int)",
                "case class Point(x: Int, y: Int)",
            ),
            (
                "type",
                "com.example.shapes.Name",
                "Name",
                "type Name = String",
                "type Name = String",
            ),
            (
                "fn",
                "com.example.shapes.helper",
                "helper",
                "def helper(): Int = 1",
                "def helper(): Int = 1",
            ),
        ],
        &["mutable", "Try", "Ok"],
    ),
    (
        "module.lua",
        include_str!("fixtures/syntax/module.lua"),
        &[
            (
                "static",
                "json",
                "json",
                "local json = require(\"json\")",
                "local json = require(\"json\")",
            ),
            (
                "static",
                "util",
                "util",
                "local util = require \"lib.util\"",
                "local util = require \"lib.util\"",
            ),
            ("static", "M", "M", "local M = {}", "local M = {}"),
            ("static", "LIMIT", "LIMIT", "LIMIT = 10", "LIMIT = 10"),
            (
                "static",
                "count",
                "count",
                "local count = 0",
                "local count = 0",
            ),
            (
                "fn",
                "M.inner.make",
                "make",
                "function M.inner.make(a)",
                "end",
            ),
            ("method", "M.method", "method", "function M:method()", "end"),
            ("fn", "helper", "helper", "local function helper()", "end"),
            (
                "fn",
                "global_fn",
                "global_fn",
                "function global_fn()",
                "end",
            ),
            (
                "fn",
                "global_fn.nested",
                "nested",
                "local function nested() end",
                "local function nested() end",
            ),
            (
                "fn",
                "M.assigned",
                "assigned",
                "M.assigned = function() end",
                "M.assigned = function() end",
            ),
            (
                "fn",
                "anon",
                "anon",
                "local anon = function() end",
                "local anon = function() end",
            ),
        ],
        &["json", "util", "setup"],
    ),
    (
        "store.dart",
        include_str!("fixtures/syntax/store.dart"),
        &[
            (
                "const",
                "limit",
                "limit",
                "const limit = 10;",
                "const limit = 10;",
            ),
            (
                "const",
                "greeting",
                "greeting",
                "final greeting = 'hi';",
                "final greeting = 'hi';",
            ),
            (
                "static",
                "counter",
                "counter",
                "var counter = 0;",
                "var counter = 0;",
            ),
            ("class", "Store", "Store", "@immutable", "}"),
            ("method", "Store.Store", "Store", "Store();", "Store();"),
            (
                "method",
                "Store.named",
                "named",
                "Store.named();",
                "Store.named();",
            ),
            (
                "method",
                "Store.put",
                "put",
                "@override",
                "  void put(int a) {}",
            ),
            (
                "method",
                "Store.size",
                "size",
                "int get size => 0;",
                "int get size => 0;",
            ),
            (
                "method",
                "Store.create",
                "create",
                "static Store create() => Store();",
                "static Store create() => Store();",
            ),
            ("class", "Shape", "Shape", "abstract class Shape {", "}"),
            ("trait", "Greets", "Greets", "mixin Greets {", "}"),
            (
                "method",
                "Greets.hello",
                "hello",
                "void hello() {}",
                "void hello() {}",
            ),
            ("impl", "Store", "", "extension StoreX on Store {", "}"),
            (
                "method",
                "Store.extra",
                "extra",
                "void extra() {}",
                "void extra() {}",
            ),
            (
                "enum",
                "Color",
                "Color",
                "enum Color { red, green }",
                "enum Color { red, green }",
            ),
            ("variant", "Color.red", "red", "red", "red"),
            ("variant", "Color.green", "green", "green", "green"),
            (
                "type",
                "Name",
                "Name",
                "typedef Name = String;",
                "typedef Name = String;",
            ),
            ("fn", "helper", "helper", "int helper() {", "}"),
        ],
        &["material", "util", "Future", "store.g"],
    ),
    (
        "inner.ex",
        include_str!("fixtures/syntax/inner.ex"),
        &[
            (
                "mod",
                "Outer.Inner",
                "Inner",
                "defmodule Outer.Inner do",
                "end",
            ),
            (
                "const",
                "Outer.Inner.limit",
                "limit",
                "@limit 10",
                "@limit 10",
            ),
            (
                "fn",
                "Outer.Inner.put",
                "put",
                "def put(a) when is_integer(a), do: a",
                "def put(a) when is_integer(a), do: a",
            ),
            (
                "fn",
                "Outer.Inner.put",
                "put",
                "def put(a), do: a",
                "def put(a), do: a",
            ),
            (
                "fn",
                "Outer.Inner.empty",
                "empty?",
                "def empty?(list), do: list == []",
                "def empty?(list), do: list == []",
            ),
            (
                "fn",
                "Outer.Inner.save",
                "save!",
                "defp save!(x) do",
                "  end",
            ),
            (
                "macro",
                "Outer.Inner.mac",
                "mac",
                "defmacro mac(x), do: x",
                "defmacro mac(x), do: x",
            ),
            (
                "mod",
                "Outer.Inner.Nested",
                "Nested",
                "defmodule Nested do",
                "  end",
            ),
            (
                "fn",
                "Outer.Inner.Nested.run",
                "run",
                "def run, do: 1",
                "def run, do: 1",
            ),
            ("interface", "Shape", "Shape", "defprotocol Shape do", "end"),
            (
                "impl",
                "Outer.Inner",
                "",
                "defimpl Shape, for: Outer.Inner do",
                "end",
            ),
            (
                "fn",
                "Outer.Inner.area",
                "area",
                "def area(_), do: 0",
                "def area(_), do: 0",
            ),
        ],
        &[
            "Helpers",
            "Alpha",
            "Beta",
            "Config",
            "Enum",
            "Logger",
            "GenServer",
        ],
    ),
    (
        "Shapes.hs",
        include_str!("fixtures/syntax/Shapes.hs"),
        &[
            ("const", "limit", "limit", "limit = 10", "limit = 10"),
            (
                "type",
                "Shape",
                "Shape",
                "data Shape = Circle Double | Square Double",
                "data Shape = Circle Double | Square Double",
            ),
            (
                "variant",
                "Shape.Circle",
                "Circle",
                "Circle Double",
                "Circle Double",
            ),
            (
                "variant",
                "Shape.Square",
                "Square",
                "Square Double",
                "Square Double",
            ),
            (
                "type",
                "Name",
                "Name",
                "newtype Name = Name String",
                "newtype Name = Name String",
            ),
            (
                "type",
                "Alias",
                "Alias",
                "type Alias = Int",
                "type Alias = Int",
            ),
            (
                "interface",
                "Area",
                "Area",
                "class Area a where",
                "  area :: a -> Double",
            ),
            (
                "impl",
                "Shape",
                "",
                "instance Area Shape where",
                "  area (Square s) = s * s",
            ),
            (
                "fn",
                "Shape.area",
                "area",
                "area (Circle r) = r * r",
                "area (Circle r) = r * r",
            ),
            (
                "fn",
                "Shape.area",
                "area",
                "area (Square s) = s * s",
                "area (Square s) = s * s",
            ),
            ("fn", "make", "make'", "make' x = x + 1", "make' x = x + 1"),
            (
                "fn",
                "helper",
                "helper",
                "helper x = let y = 1 in x + y",
                "    local = 2",
            ),
            ("fn", "<+>", "<+>", "a <+> b = a + b", "a <+> b = a + b"),
        ],
        &["sortBy", "M", "Maybe"],
    ),
];

/// Exact units, qualified names, name ranges and import keys per fixture;
/// documents tile; the fixture's first half (a malformed source) still
/// parses and tiles; a stored name drops its `?`/`!`/`'` suffix.
#[test]
fn new_languages_give_exact_units_qnames_name_ranges_and_import_keys() {
    let mut languages = std::collections::BTreeSet::new();
    for &(file, source, rows, imports) in FIXTURES {
        let lang = Lang::from_path(file).unwrap_or_else(|| panic!("{file} is mapped"));
        languages.insert(lang.tag());
        let units = syntax::units(source, lang);
        let found: Vec<[&str; 5]> = units
            .iter()
            .map(|unit| {
                let text = &source[unit.start..unit.end];
                [
                    unit.kind.as_str(),
                    unit.qname.as_deref().unwrap_or(""),
                    unit.name_range
                        .map_or("", |(start, end)| &source[start..end]),
                    text.lines().next().unwrap_or(""),
                    text.lines().last().unwrap_or(""),
                ]
            })
            .collect();
        let want: Vec<[&str; 5]> = rows
            .iter()
            .map(|&(kind, qname, name, first, last)| [kind, qname, name, first, last])
            .collect();
        assert_eq!(found, want, "{file}");
        for unit in &units {
            let name = unit.name.as_deref().unwrap_or("");
            assert!(
                !name.ends_with(['?', '!', '\'']) || unit.name_range.is_none(),
                "{file}: {name} keeps its suffix"
            );
        }
        assert_eq!(
            syntax::index(source, Some(lang)).unwrap().imports,
            imports,
            "{file}"
        );
        assert_tiles(source, Some(lang));
        let mut half = source.len() / 2;
        while !source.is_char_boundary(half) {
            half -= 1;
        }
        assert_tiles(&source[..half], Some(lang));
    }
    assert_eq!(languages.len(), 15, "{languages:?}");
    // Stored names are stripped; name ranges keep the suffix as written.
    let ruby = syntax::units("def empty?\nend\n", Lang::Ruby).remove(0);
    assert_eq!(
        (ruby.name.as_deref(), ruby.name_range),
        (Some("empty"), Some((4, 10)))
    );
    let haskell = syntax::units("x'' = 1\n", Lang::Haskell).remove(0);
    assert_eq!(haskell.name.as_deref(), Some("x"));
}

/// Deeply nested and malformed inputs parse without panic on the 2 MiB
/// stack of indexing's build threads and still tile.
#[test]
fn deeply_nested_new_language_inputs_tile_on_a_two_mib_stack() {
    let langs = [
        Lang::CSharp,
        Lang::FSharp,
        Lang::FSharpSignature,
        Lang::VbNet,
        Lang::Php,
        Lang::Perl,
        Lang::Bash,
        Lang::PowerShell,
        Lang::Ruby,
        Lang::Kotlin,
        Lang::Swift,
        Lang::Scala,
        Lang::Lua,
        Lang::Dart,
        Lang::Elixir,
        Lang::Haskell,
    ];
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            for lang in langs {
                for source in [
                    format!("x = {}1{}\n", "(".repeat(1000), ")".repeat(1000)),
                    format!("x = {}1\n", "{ [".repeat(1000)),
                ] {
                    assert_tiles(&source, Some(lang));
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

/// A parse over the work budget (context-v2 § Languages; the spec's
/// 4,000-deep Haskell `let`) stops at the same byte on 1 and 8 build threads
/// and becomes a named failure with the plain blocks of an unmapped source;
/// the other sources are parsed as usual.
#[test]
fn a_parse_over_the_work_budget_stops_at_the_same_byte_on_one_and_eight_threads() {
    use context_foundry::store::index_hooks::{self, Hooks};
    use context_foundry::{Control, Engine};
    let deep = format!(
        "x = {}1{}\n",
        "let { a = ".repeat(4000),
        " } in a".repeat(4000)
    );
    let mut runs = Vec::new();
    for threads in [1, 8] {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("ws");
        std::fs::create_dir(&root).unwrap();
        let store = fixture.path().join("store");
        let mut engine = Engine::initialize(&store, &root).unwrap();
        engine.replace_source("deep/let.hs", &deep).unwrap();
        for i in 0..15 {
            engine
                .replace_source(&format!("src/f{i:02}.hs"), &format!("f{i} x = x\n"))
                .unwrap();
        }
        index_hooks::install(Hooks {
            threads: Some(threads),
            ..Hooks::default()
        });
        assert_eq!(engine.refresh(&Control::unbounded()).unwrap(), (16, 0));
        index_hooks::clear();
        let failures = engine.take_parse_failures();
        let status = engine.status().unwrap();
        let documents = index_hooks::committed_documents(&store, "deep/let.hs");
        let normal = index_hooks::committed_documents(&store, "src/f00.hs").len();
        runs.push((failures, status.parse_failures, documents, normal));
    }
    assert_eq!(runs[0], runs[1], "1 and 8 threads");
    let (failures, count, documents, normal) = &runs[0];
    assert_eq!(failures.count, 1);
    let prefix = format!(
        "deep/let.hs: parse_stopped: exceeded the parse work budget of {} units at byte ",
        syntax::PARSE_WORK_BUDGET
    );
    let at: usize = failures.samples[0]
        .strip_prefix(&prefix)
        .unwrap_or_else(|| panic!("{:?}", failures.samples))
        .parse()
        .unwrap();
    assert!(0 < at && at < deep.len(), "{at}");
    assert_eq!(*count, Some(1), "status counts the unparsed source");
    let starts: Vec<u64> = documents.iter().map(|(_, start)| *start).collect();
    let blocks: Vec<u64> = syntax::documents(&deep, None)
        .iter()
        .map(|document| document.start as u64)
        .collect();
    assert_eq!(starts, blocks, "the plain blocks of an unmapped source");
    assert_eq!(*normal, 1, "a normal source keeps its unit");
}

// --- 001 T008 review: scanner limits (M1), wide lists (M2), binding groups
// (M3, M4), Dart local functions (M5), PowerShell module operands (M6)

const HAZARD: &str = "FOUNDRY_TEST_SCANNER_HAZARD";
const HAZARDS: [&str; 10] = [
    "fsharp-comments",
    "fsharp-comments-indexed",
    "perl-brackets",
    "perl-heredoc-identifier",
    "perl-heredoc-line",
    "python-indentation",
    "kotlin-trailing-at",
    "kotlin-forced-end",
    "ruby-heredoc-1023",
    "ruby-heredoc-300",
];

/// `source` is a stopped parse over `limit` and falls back to the plain
/// blocks of an unmapped source.
fn assert_stopped(source: &str, lang: Lang, limit: syntax::ScannerLimit) {
    let stopped = syntax::index(source, Some(lang)).unwrap_err();
    assert_eq!(stopped.stop, syntax::Stop::Scanner(limit), "{stopped}");
    assert!(stopped.at < source.len());
    assert_eq!(
        syntax::documents(source, Some(lang)),
        syntax::documents(source, None)
    );
}

/// Runs one hazard (the child's side of the test below).
fn run_hazard(case: &str) {
    use syntax::ScannerLimit::*;
    let nested = |open: &str, close: &str, depth: usize| open.repeat(depth) + &close.repeat(depth);
    let ruby = |word: usize| {
        let word = "A".repeat(word);
        format!("X = <<{word}\nbody\n{word}\n\ndef after\nend\n")
    };
    match case {
        // The review's trigger: 250,000 nested comments, about 1 MB.
        "fsharp-comments" => {
            let source = nested("(*", "*)", 250_000) + "\nlet x = 1\n";
            assert_stopped(&source, Lang::FSharp, FSharpCommentDepth);
            assert_stopped(&source, Lang::FSharpSignature, FSharpCommentDepth);
        }
        // The same through indexing's own build threads: a named failure.
        "fsharp-comments-indexed" => {
            use context_foundry::{Control, Engine};
            let fixture = tempfile::tempdir().unwrap();
            let root = fixture.path().join("ws");
            std::fs::create_dir(&root).unwrap();
            let store = fixture.path().join("store");
            let mut engine = Engine::initialize(&store, &root).unwrap();
            let source = nested("(*", "*)", 250_000) + "\nlet x = 1\n";
            engine.replace_source("deep.fs", &source).unwrap();
            engine.replace_source("ok.fs", "let f x = x\n").unwrap();
            assert_eq!(engine.refresh(&Control::unbounded()).unwrap(), (2, 0));
            let failures = engine.take_parse_failures();
            assert_eq!(
                failures.samples,
                [
                    "deep.fs: parse_stopped: over the scanner limit: F# comments nest deeper than 8192 at byte 16384"
                ]
            );
        }
        "perl-brackets" => assert_stopped(
            &format!("my $x = q{{{}}};\n", nested("{", "}", 250_000)),
            Lang::Perl,
            PerlBracketDepth,
        ),
        "perl-heredoc-identifier" => assert_stopped(
            &format!("my $x = <<{};\n", "A".repeat(5000)),
            Lang::Perl,
            PerlHeredocWord,
        ),
        "perl-heredoc-line" => assert_stopped(
            &format!("my $x = <<EOT;\n{}\nEOT\n", "a".repeat(5000)),
            Lang::Perl,
            PerlHeredocWord,
        ),
        // 600 indentation levels, then strings at the deepest.
        "python-indentation" => {
            let mut source: String = (0..600)
                .map(|level| format!("{}if x:\n", " ".repeat(level)))
                .collect();
            source += &format!("{}y = 'a' + \"b\"\n", " ".repeat(600));
            assert_stopped(&source, Lang::Python, PythonIndentWidths);
        }
        "kotlin-trailing-at" => {
            for source in [
                "class A {\n  val x: Int\n    @Foo",
                "class A {\n  val x: Int\n    @Foo(",
            ] {
                assert_stopped(source, Lang::Kotlin, KotlinTrailingAt);
            }
        }
        // Every budget, so that some run out inside the run after `@`: the
        // forced end of input reads as a line break, and each parse ends.
        "kotlin-forced-end" => {
            let source = format!(
                "class A {{\n  val x: Int\n    @{}\n  val y = 1\n}}\n",
                "a".repeat(300)
            );
            let mut budget = 1;
            loop {
                syntax::parse_hooks::set_budget(Some(budget));
                let done = syntax::index(&source, Some(Lang::Kotlin)).is_ok();
                syntax::parse_hooks::set_budget(None);
                if done {
                    break;
                }
                budget += 1;
            }
            assert!(budget > 300, "{budget}");
        }
        // The fork's fix (Cargo.toml [patch.crates-io]): a state of exactly
        // 1,023 bytes, and a word over 255 bytes, parse.
        "ruby-heredoc-1023" | "ruby-heredoc-300" => {
            let source = ruby(if case.ends_with("1023") { 1019 } else { 300 });
            assert_eq!(
                units(&source, Lang::Ruby)
                    .iter()
                    .map(|(kind, qname, _)| (*kind, qname.clone().unwrap_or_default()))
                    .collect::<Vec<_>>(),
                [("const", "X".to_owned()), ("method", "after".to_owned())]
            );
            assert_tiles(&source, Some(Lang::Ruby));
        }
        other => panic!("unknown hazard {other}"),
    }
}

/// Each scanner hazard of the 001 T008 audit (review M1) runs in a child
/// process on a 2 MiB stack: the process must neither crash nor hang, and
/// the source falls back (a limit) or parses (the patched Ruby scanner).
#[test]
fn scanner_hazards_fall_back_in_a_child_process_on_a_two_mib_stack() {
    if let Ok(case) = std::env::var(HAZARD) {
        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || run_hazard(&case))
            .unwrap()
            .join()
            .unwrap();
        println!("hazard finished");
        return;
    }
    for case in HAZARDS {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "scanner_hazards_fall_back_in_a_child_process_on_a_two_mib_stack",
                "--nocapture",
            ])
            .env(HAZARD, case)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let started = std::time::Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if started.elapsed() > std::time::Duration::from_secs(300) {
                child.kill().unwrap();
                panic!("{case}: no end in 300 s");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        let output = child.wait_with_output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            status.success() && stdout.contains("hazard finished"),
            "{case}: {status:?}\n{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Wide lists are read once per list, not once per member (review M2): a
/// 20,000-constructor Haskell sum type, Go `var` block, JavaScript
/// declaration and shell `export` give one unit per member; a lone
/// constructor stays part of its type.
#[test]
fn wide_lists_give_a_unit_per_member() {
    let count = 20_000;
    let haskell = format!(
        "data T = {}\n",
        (0..count)
            .map(|i| format!("C{i:05}"))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    let go = format!(
        "package p\n\nvar (\n{})\n",
        (0..count)
            .map(|i| format!("\tv{i} = {i}\n"))
            .collect::<String>()
    );
    let javascript = format!(
        "var {};\n",
        (0..count)
            .map(|i| format!("f{i} = () => {i}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let shell = format!(
        "export {}\n",
        (0..count)
            .map(|i| format!("V{i}={i}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    for (lang, source, kind, members) in [
        (Lang::Haskell, &haskell, "variant", count),
        (Lang::Go, &go, "static", count),
        (Lang::JavaScript, &javascript, "fn", count),
        (Lang::Bash, &shell, "static", count),
    ] {
        let started = std::time::Instant::now();
        let found = syntax::units(source, lang);
        let elapsed = started.elapsed();
        assert_eq!(
            found
                .iter()
                .filter(|unit| unit.kind.as_str() == kind)
                .count(),
            members,
            "{lang:?}"
        );
        println!("{lang:?}: {} units in {elapsed:?}", found.len());
    }
    assert_eq!(
        units("data P = P Int\n", Lang::Haskell),
        [named("type", "P", "data P = P Int")]
    );
}

/// Each binding of an F# `let rec … and …` group is a unit with its own
/// range (review M3): a function at any depth, a value at module level.
#[test]
fn fsharp_recursive_groups_give_a_unit_per_binding() {
    let source = "let rec even n = n = 0 || odd (n - 1)\nand odd n = n <> 0 && even (n - 1)\n\nmodule M =\n    let rec a x = b x\n    and b x = a x\n    and c = 3\n";
    assert_eq!(
        units(source, Lang::FSharp),
        [
            named("fn", "even", "let rec even n = n = 0 || odd (n - 1)"),
            named("fn", "odd", "and odd n = n <> 0 && even (n - 1)"),
            named("mod", "M", span(source, "module M", "and c = 3")),
            named("fn", "M.a", "let rec a x = b x"),
            named("fn", "M.b", "and b x = a x"),
            named("const", "M.c", "and c = 3"),
        ]
    );
    for unit in syntax::units(source, Lang::FSharp) {
        let (start, end) = unit.name_range.unwrap();
        assert_eq!(Some(&source[start..end]), unit.name.as_deref());
    }
    // A local group: its functions are units, its value is not.
    let local =
        "let outer () =\n    let rec f x = g x\n    and g x = f x\n    and v = 1\n    f 0\n";
    let qnames: Vec<_> = units(local, Lang::FSharp)
        .into_iter()
        .map(|(_, qname, _)| qname.unwrap())
        .collect();
    assert_eq!(qnames, ["outer", "outer.f", "outer.g"]);
    assert_tiles(source, Some(Lang::FSharp));
    assert_tiles(local, Some(Lang::FSharp));
}

/// Grouped module-level bindings are a unit each (review M4): Swift
/// `let a = 1, b = 2` (a tuple pattern among them is skipped, locals are
/// none) and shell `A=1 B=2`; a command's prefix assignment and a
/// function's assignments are none.
#[test]
fn grouped_swift_and_shell_bindings_give_a_unit_each() {
    let swift = "let first = 1, second = 2\nvar count: Int = 0, total = 1\nlet (x, y) = (1, 2), z = 3\nfunc f() {\n    let a = 1, b = 2\n}\n";
    assert_eq!(
        units(swift, Lang::Swift),
        [
            named("const", "first", "let first = 1"),
            named("const", "second", "second = 2"),
            named("static", "count", "var count: Int = 0"),
            named("static", "total", "total = 1"),
            named("const", "z", "z = 3"),
            named("fn", "f", span(swift, "func f", "}")),
        ]
    );
    let shell = "FIRST=1 SECOND=2\nFOO=1 cmd\nf() {\n  A=1 B=2\n  local C=3\n}\nexport X=1 Y=2\nreadonly R=1 S=2\ndeclare -r T=1\n";
    assert_eq!(
        units(shell, Lang::Bash),
        [
            named("static", "FIRST", "FIRST=1"),
            named("static", "SECOND", "SECOND=2"),
            named("fn", "f", span(shell, "f()", "}")),
            named("static", "X", "X=1"),
            named("static", "Y", "Y=2"),
            named("const", "R", "R=1"),
            named("const", "S", "S=2"),
            named("const", "T", "declare -r T=1"),
        ]
    );
    assert_tiles(swift, Some(Lang::Swift));
    assert_tiles(shell, Some(Lang::Bash));
}

/// A named Dart local function is a function unit nested in its enclosing
/// one, with its body; a local variable is not a unit (review M5).
#[test]
fn dart_local_functions_are_units() {
    let source = "void outer() {\n  int inner() => 1;\n  var x = 2;\n  void deeper() {\n    String innermost() => '';\n  }\n  print(inner());\n}\n";
    assert_eq!(
        units(source, Lang::Dart),
        [
            named("fn", "outer", source.trim_end()),
            named("fn", "outer.inner", "int inner() => 1;"),
            named("fn", "outer.deeper", span(source, "void deeper", "\n  }")),
            named("fn", "outer.deeper.innermost", "String innermost() => '';"),
        ]
    );
    let inner = &syntax::units(source, Lang::Dart)[1];
    let (start, end) = inner.name_range.unwrap();
    assert_eq!(&source[start..end], "inner");
    let (start, end) = inner.body.unwrap();
    assert_eq!(&source[start..end], "=> 1;");
    assert_tiles(source, Some(Lang::Dart));
}

/// A PowerShell module operand gives its key quoted or not, by position or
/// as `-Name`'s value; another parameter's value and other commands give
/// none (review M6).
#[test]
fn powershell_module_operands_give_keys_quoted_or_not() {
    for (source, keys) in [
        ("Import-Module './Store.psm1'\n", &["Store"][..]),
        ("using module './Store.psm1'\n", &["Store"]),
        ("using module \"C:\\Mods\\Store.psm1\"\n", &["Store"]),
        ("Import-Module -Name \"Store\"\n", &["Store"]),
        (
            "Import-Module -Name './lib/Tools.psd1' -Force\n",
            &["Tools"],
        ),
        ("Import-Module -Force 'Store'\n", &["Store"]),
        ("Import-Module -Prefix X Store\n", &["Store"]),
        ("Import-Module 'A', 'B'\n", &["A", "B"]),
        ("Import-Module Tools\n", &["Tools"]),
        ("using module Tools\n", &["Tools"]),
        ("using namespace System.IO\n", &["IO"]),
        ("Write-Host 'Store.psm1'\n", &[]),
        ("Get-Module -Name 'Store'\n", &[]),
    ] {
        assert_eq!(
            syntax::index(source, Some(Lang::PowerShell))
                .unwrap()
                .imports,
            keys,
            "{source}"
        );
    }
}

/// New languages' address qualifiers come from the syntax tree (001 T007's
/// name addresses): scoped and qualified names give each part, a receiver
/// or extended type its own parts, never its arguments.
#[test]
fn new_language_qualifiers_come_from_the_tree() {
    for (lang, source, qname, want) in [
        (
            Lang::Ruby,
            "class Outer::Inner::Store\n  def get\n  end\nend\n",
            "Outer::Inner::Store::get",
            &["outer", "inner", "store"][..],
        ),
        (
            Lang::Lua,
            "function M.inner:make()\nend\n",
            "M.inner.make",
            &["m", "inner"],
        ),
        (
            Lang::Php,
            "<?php\nnamespace App\\Models;\nfunction f() {}\n",
            "App.Models.f",
            &["app", "models"],
        ),
        (
            Lang::Perl,
            include_str!("fixtures/syntax/shape.pl"),
            "Outer::Shape::new",
            &["outer", "shape"],
        ),
        (
            Lang::CSharp,
            "namespace Outer.Space { class C {} }\n",
            "Outer.Space.C",
            &["outer", "space"],
        ),
        (
            Lang::Elixir,
            "defmodule Shapes.Inner do\n  def area(x), do: x\nend\n",
            "Shapes.Inner.area",
            &["shapes", "inner"],
        ),
        (
            Lang::Kotlin,
            "fun List<Int>.total() = 0\n",
            "List<Int>.total",
            &["list"],
        ),
        (
            Lang::Swift,
            "extension Outer.Inner {\n    func f() {}\n}\n",
            "Outer.Inner.f",
            &["outer", "inner"],
        ),
        (
            Lang::Dart,
            "extension on List<int> {\n  void f() {}\n}\n",
            "List<int>.f",
            &["list"],
        ),
        (
            Lang::Haskell,
            "instance Show (Maybe a) where\n  show _ = \"\"\n",
            "(Maybe a).show",
            &["maybe"],
        ),
    ] {
        let found = syntax::units(source, lang);
        let unit = found
            .iter()
            .find(|unit| unit.qname.as_deref() == Some(qname))
            .unwrap_or_else(|| panic!("no {qname} in {found:#?}"));
        assert_eq!(unit.qualifiers, want, "{qname}");
    }
}
