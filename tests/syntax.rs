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
        ("a.bash", "bash", false),
        ("a.sql", "sql", false),
    ] {
        let lang = Lang::from_path(path).unwrap_or_else(|| panic!("{path} is mapped"));
        assert_eq!((lang.tag(), lang.has_units()), (tag, has_units), "{path}");
    }
    for unmapped in ["a.txt", "Makefile", ".rs", "dir.rs/file", "a.RS"] {
        assert_eq!(Lang::from_path(unmapped), None, "{unmapped}");
    }
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
        assert_eq!(syntax::index(source, Some(lang)).imports, keys, "{lang:?}");
    }
    // Keys are distinct; documents are the plain document tiling.
    let source = "use a::X;\nuse b::X;\nfn f() {}\n";
    let index = syntax::index(source, Some(Lang::Rust));
    assert_eq!(index.imports, ["X"]);
    assert_eq!(index.documents, syntax::documents(source, Some(Lang::Rust)));
    assert!(syntax::index(source, None).imports.is_empty());
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
