//! Import keys: the module, package and file names each language's import
//! syntax names, which door ordering and reference scopes match. The shared
//! tree helpers and `without_extension`, which path segments also use, stay in
//! the parent module.
use super::*;

/// The import keys one node contributes (context-v2 § Doors import keys):
/// for an import statement, the bound names it introduces — a named import
/// or its alias, the last segment of a `use`/`import` path, the names of
/// `from m import a, b`, the last segment of a `using`/`open`/`Imports`
/// namespace or declaration, an `#include "x/y.h"` as `y` — and a
/// `require`/`source`/`include` path's file stem (a Lua module path's last
/// segment). Glob imports (`use m::*`, `import a.*`, `from m import *`,
/// `import x._`) and Go's `.` and `_` imports give no key.
pub(super) fn import_keys(
    lang: Lang,
    node: tree_sitter::Node,
    source: &[u8],
    out: &mut Vec<String>,
) {
    let text = |node: tree_sitter::Node| node.utf8_text(source).ok().map(str::to_owned);
    let mut cursor = node.walk();
    match (lang, node.kind()) {
        (Lang::Rust, "use_declaration") => {
            if let Some(argument) = node.child_by_field_name("argument") {
                rust_use_keys(argument, source, out);
            }
        }
        (Lang::Python, "import_statement" | "import_from_statement") => {
            for name in node.children_by_field_name("name", &mut cursor) {
                let bound = match name.kind() {
                    "aliased_import" => name.child_by_field_name("alias"),
                    // A dotted name binds its last segment.
                    _ => last_named_child(name),
                };
                out.extend(bound.and_then(text));
            }
        }
        (Lang::TypeScript | Lang::Tsx | Lang::JavaScript, "import_statement") => {
            for child in node.named_children(&mut cursor) {
                match child.kind() {
                    "import_clause" => {
                        let mut parts = child.walk();
                        for part in child.named_children(&mut parts) {
                            match part.kind() {
                                "identifier" => out.extend(text(part)),
                                "namespace_import" => {
                                    out.extend(last_named_child(part).and_then(text));
                                }
                                "named_imports" => {
                                    let mut specifiers = part.walk();
                                    for specifier in part.named_children(&mut specifiers) {
                                        let bound =
                                            specifier.child_by_field_name("alias").or_else(|| {
                                                specifier
                                                    .child_by_field_name("name")
                                                    .filter(|name| name.kind() == "identifier")
                                            });
                                        out.extend(bound.and_then(text));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "import_require_clause" => {
                        let mut parts = child.walk();
                        let bound = child
                            .named_children(&mut parts)
                            .find(|part| part.kind() == "identifier");
                        out.extend(bound.and_then(text));
                    }
                    _ => {}
                }
            }
        }
        (Lang::TypeScript | Lang::Tsx | Lang::JavaScript, "call_expression") => {
            let is_require = node
                .child_by_field_name("function")
                .is_some_and(|function| text(function).as_deref() == Some("require"));
            let argument = node
                .child_by_field_name("arguments")
                .filter(|arguments| arguments.named_child_count() == 1)
                .and_then(|arguments| arguments.named_child(0))
                .filter(|argument| argument.kind() == "string");
            if is_require && let Some(argument) = argument {
                out.extend(text(argument).and_then(|path| file_stem(&path)));
            }
        }
        (Lang::Go, "import_spec") => match node.child_by_field_name("name") {
            Some(name) if name.kind() == "package_identifier" => out.extend(text(name)),
            Some(_) => {}
            None => {
                let path = node.child_by_field_name("path").and_then(text);
                out.extend(path.and_then(|path| {
                    let path = path.trim_matches(|c| c == '"' || c == '`');
                    let last = path.rsplit('/').next().unwrap_or(path);
                    (!last.is_empty()).then(|| last.to_owned())
                }));
            }
        },
        (Lang::C | Lang::Cpp, "preproc_include") => {
            let path = node
                .child_by_field_name("path")
                .filter(|path| matches!(path.kind(), "string_literal" | "system_lib_string"));
            out.extend(path.and_then(text).and_then(|path| file_stem(&path)));
        }
        (Lang::Cpp, "using_declaration") => {
            let named = node
                .named_children(&mut cursor)
                .filter(|child| matches!(child.kind(), "identifier" | "qualified_identifier"))
                .last();
            out.extend(named.and_then(text).and_then(|name| {
                let last = name.rsplit("::").next().unwrap_or(&name).trim();
                (!last.is_empty()).then(|| last.to_owned())
            }));
        }
        (Lang::Java, "import_declaration") => {
            let mut glob = false;
            let mut path = None;
            for child in node.named_children(&mut cursor) {
                match child.kind() {
                    "asterisk" => glob = true,
                    "identifier" => path = Some(child),
                    "scoped_identifier" => path = child.child_by_field_name("name"),
                    _ => {}
                }
            }
            if !glob {
                out.extend(path.and_then(text));
            }
        }
        (Lang::CSharp, "using_directive") => match node.child_by_field_name("name") {
            Some(alias) => out.extend(text(alias)),
            None => out.extend(
                node.named_children(&mut cursor)
                    .filter(|child| {
                        matches!(
                            child.kind(),
                            "identifier" | "qualified_name" | "alias_qualified_name"
                        )
                    })
                    .last()
                    .and_then(text)
                    .and_then(|path| last_segment(&path, &[".", "::"])),
            ),
        },
        (Lang::FSharp | Lang::FSharpSignature, "import_decl") => out.extend(
            named_child_of(node, &["long_identifier"])
                .and_then(text)
                .and_then(|path| last_segment(&path, &["."])),
        ),
        (Lang::FSharp, "fsi_directive_decl") => {
            if text(node).is_some_and(|directive| directive.starts_with("#load")) {
                for path in node.named_children(&mut cursor) {
                    out.extend(text(path).and_then(|path| file_stem(&path)));
                }
            }
        }
        (Lang::VbNet, "imports_statement") => {
            for path in node.children_by_field_name("namespace", &mut cursor) {
                out.extend(text(path).and_then(|path| last_segment(&path, &["."])));
            }
        }
        (Lang::Php, "namespace_use_clause") => {
            let bound = node
                .child_by_field_name("alias")
                .or_else(|| named_child_of(node, &["qualified_name", "name"]));
            out.extend(
                bound
                    .and_then(text)
                    .and_then(|name| last_segment(&name, &["\\"])),
            );
        }
        (
            Lang::Php,
            "include_expression"
            | "include_once_expression"
            | "require_expression"
            | "require_once_expression",
        ) => {
            // A literal path, or one concatenated onto `__DIR__`.
            let path = node.named_child(0).and_then(|path| match path.kind() {
                "binary_expression" => path.child_by_field_name("right"),
                _ => Some(path),
            });
            out.extend(
                path.filter(|path| matches!(path.kind(), "string" | "encapsed_string"))
                    .and_then(text)
                    .and_then(|path| file_stem(&path)),
            );
        }
        (Lang::Perl, "use_no_statement" | "require_statement") => {
            let used = node.kind() == "require_statement" || has_child_kind(node, "use");
            if used {
                out.extend(
                    node.child_by_field_name("package_name")
                        .and_then(text)
                        .and_then(|name| last_segment(&name, &["::"])),
                );
            }
        }
        (Lang::Perl, "use_parent_statement") => {
            for child in node.named_children(&mut cursor) {
                let names: Vec<tree_sitter::Node> = match child.kind() {
                    "word_list_qw" => named_children(child)
                        .filter(|item| item.kind() == "list_item")
                        .collect(),
                    kind if kind.starts_with("string") => vec![child],
                    _ => Vec::new(),
                };
                for name in names {
                    out.extend(
                        text(name)
                            .and_then(|name| last_segment(name.trim_matches(['\'', '"']), &["::"])),
                    );
                }
            }
        }
        (Lang::Bash, "command") => {
            let sources = node
                .child_by_field_name("name")
                .and_then(text)
                .is_some_and(|name| matches!(name.as_str(), "source" | "."));
            if sources {
                out.extend(
                    node.child_by_field_name("argument")
                        .and_then(text)
                        .and_then(|path| file_stem(&path)),
                );
            }
        }
        (Lang::PowerShell, "command") => {
            let module = |name: &str| {
                if name.contains(['/', '\\'])
                    || name.to_ascii_lowercase().ends_with(".psm1")
                    || name.to_ascii_lowercase().ends_with(".psd1")
                    || name.to_ascii_lowercase().ends_with(".ps1")
                    || name.to_ascii_lowercase().ends_with(".dll")
                {
                    file_stem(name)
                } else {
                    last_segment(name, &["."])
                }
            };
            // `. ./x.ps1` dot-sources a script. Only a static name gives a
            // key: a bare path or a string holding no variable or
            // subexpression (`. $path`, `. "$name"`, `. "$(Get-X)"` give
            // none; 001 T008 review R8).
            if named_child_of(node, &["command_invokation_operator"])
                .and_then(text)
                .as_deref()
                == Some(".")
            {
                let script = node
                    .child_by_field_name("command_name")
                    .and_then(|name| match name.kind() {
                        "command_name_expr" if name.named_child_count() == 1 => name.named_child(0),
                        _ => Some(name),
                    })
                    .filter(|name| match name.kind() {
                        // A bare name may embed a variable (`./lib/$name.ps1`).
                        "command_name" => text(*name).is_some_and(|path| !path.contains('$')),
                        "string_literal" => !named_children(*name).any(|string| {
                            matches!(
                                string.kind(),
                                "expandable_string_literal" | "expandable_here_string_literal"
                            ) && string.named_child_count() > 0
                        }),
                        _ => false,
                    });
                out.extend(
                    script
                        .and_then(text)
                        .and_then(|path| file_stem(path.trim_matches(['\'', '"']))),
                );
                return;
            }
            let Some(command) = node.child_by_field_name("command_name").and_then(text) else {
                return;
            };
            let elements: Vec<PowerShellElement> = node
                .child_by_field_name("command_elements")
                .map(|elements| {
                    named_children(elements)
                        .filter_map(|element| powershell_element(element, source))
                        .collect()
                })
                .unwrap_or_default();
            match command.to_ascii_lowercase().as_str() {
                "using" => {
                    if let [
                        PowerShellElement::Argument(kind),
                        PowerShellElement::Argument(names),
                        ..,
                    ] = elements.as_slice()
                        && let (Some(kind), Some(name)) = (kind.first(), names.first())
                    {
                        if kind.eq_ignore_ascii_case("namespace") {
                            out.extend(last_segment(name, &["."]));
                        } else if kind.eq_ignore_ascii_case("module") {
                            out.extend(module(name));
                        }
                    }
                }
                // The `-Name` operand, else the first argument no other
                // parameter takes as its value.
                "import-module" => {
                    let mut elements = elements.iter();
                    while let Some(element) = elements.next() {
                        match element {
                            PowerShellElement::Parameter(parameter) if parameter == "name" => {
                                if let Some(PowerShellElement::Argument(names)) = elements.next() {
                                    out.extend(names.iter().filter_map(|name| module(name)));
                                }
                                return;
                            }
                            PowerShellElement::Parameter(parameter)
                                if IMPORT_MODULE_SWITCHES.contains(&parameter.as_str()) => {}
                            PowerShellElement::Parameter(_) => {
                                elements.next();
                            }
                            PowerShellElement::Argument(names) => {
                                out.extend(names.iter().filter_map(|name| module(name)));
                                return;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        (Lang::Ruby, "call") => {
            if node.child_by_field_name("receiver").is_some() {
                return;
            }
            let method = node.child_by_field_name("method").and_then(text);
            let loads = matches!(
                method.as_deref(),
                Some("require" | "require_relative" | "load" | "autoload")
            );
            if loads {
                out.extend(
                    node.child_by_field_name("arguments")
                        .and_then(|arguments| {
                            named_children(arguments).find(|argument| argument.kind() == "string")
                        })
                        .and_then(text)
                        .and_then(|path| file_stem(&path)),
                );
            }
        }
        (Lang::Kotlin, "import") => {
            if has_child_kind(node, "*") {
                return;
            }
            let alias = named_child_of(node, &["identifier"]);
            out.extend(match alias {
                Some(alias) => text(alias),
                None => named_child_of(node, &["qualified_identifier"])
                    .and_then(last_named_child)
                    .and_then(text),
            });
        }
        (Lang::Swift, "import_declaration") => out.extend(
            named_child_of(node, &["identifier"])
                .and_then(last_named_child)
                .and_then(text),
        ),
        (Lang::Scala, "import_declaration") => {
            let mut path_last = None;
            let mut bound = Vec::new();
            let mut glob = false;
            let renamed = |renamed: tree_sitter::Node| {
                renamed
                    .child_by_field_name("alias")
                    .and_then(text)
                    .filter(|alias| alias != "_")
            };
            for child in node.named_children(&mut cursor) {
                match child.kind() {
                    "identifier" => path_last = Some(child),
                    "namespace_wildcard" => glob = true,
                    "arrow_renamed_identifier" | "as_renamed_identifier" => {
                        bound.extend(renamed(child));
                    }
                    "namespace_selectors" => {
                        for selector in named_children(child) {
                            match selector.kind() {
                                "identifier" => bound.extend(text(selector)),
                                "arrow_renamed_identifier" | "as_renamed_identifier" => {
                                    bound.extend(renamed(selector));
                                }
                                _ => {}
                            }
                        }
                        glob = true;
                    }
                    _ => {}
                }
            }
            if bound.is_empty() && !glob {
                bound.extend(path_last.and_then(text));
            }
            out.extend(bound);
        }
        (Lang::Lua, "function_call") => {
            let function = node.child_by_field_name("name").and_then(text);
            let path = node
                .child_by_field_name("arguments")
                .and_then(|arguments| {
                    named_children(arguments).find(|argument| argument.kind() == "string")
                })
                .and_then(text);
            match function.as_deref() {
                Some("require") => out.extend(path.and_then(|path| {
                    last_segment(path.trim_matches(['"', '\'', '[', ']']), &[".", "/"])
                })),
                Some("dofile" | "loadfile") => out.extend(path.and_then(|path| file_stem(&path))),
                _ => {}
            }
        }
        (Lang::Dart, "import_specification") => {
            if let Some(alias) = node.child_by_field_name("alias") {
                out.extend(text(alias));
                return;
            }
            let shown: Vec<String> = named_children(node)
                .filter(|combinator| {
                    combinator.kind() == "combinator"
                        && text(*combinator).is_some_and(|text| text.starts_with("show"))
                })
                .flat_map(named_children)
                .filter_map(text)
                .collect();
            if shown.is_empty() {
                out.extend(
                    node.child_by_field_name("uri")
                        .and_then(text)
                        .and_then(|uri| dart_stem(&uri)),
                );
            } else {
                out.extend(shown);
            }
        }
        (Lang::Dart, "part_directive") => {
            out.extend(
                node.child_by_field_name("uri")
                    .and_then(text)
                    .and_then(|uri| dart_stem(&uri)),
            );
        }
        (Lang::Elixir, "call") => {
            let directive = node.child_by_field_name("target").and_then(text);
            if !matches!(
                directive.as_deref(),
                Some("alias" | "import" | "require" | "use")
            ) {
                return;
            }
            let Some(arguments) = named_child_of(node, &["arguments"]) else {
                return;
            };
            let alias = named_child_of(arguments, &["keywords"]).and_then(|keywords| {
                named_children(keywords)
                    .find(|pair| {
                        pair.child_by_field_name("key")
                            .and_then(text)
                            .is_some_and(|key| key.trim() == "as:")
                    })
                    .and_then(|pair| pair.child_by_field_name("value"))
            });
            if let Some(alias) = alias {
                out.extend(text(alias));
                return;
            }
            let Some(first) = arguments.named_child(0) else {
                return;
            };
            let modules: Vec<tree_sitter::Node> = match first.kind() {
                "alias" => vec![first],
                // `alias Outer.{Alpha, Beta}`
                "dot" => first
                    .child_by_field_name("right")
                    .filter(|right| right.kind() == "tuple")
                    .map(|tuple| named_children(tuple).collect())
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            for module in modules {
                out.extend(text(module).and_then(|name| last_segment(&name, &["."])));
            }
        }
        (Lang::Haskell, "import") => {
            let names = node
                .child_by_field_name("names")
                .filter(|names| !has_child_kind(*names, "hiding"));
            if let Some(names) = names {
                let mut listed = names.walk();
                for name in names.children_by_field_name("name", &mut listed) {
                    out.extend(name.named_child(0).and_then(text));
                }
                return;
            }
            let module = node
                .child_by_field_name("alias")
                .or_else(|| node.child_by_field_name("module"));
            out.extend(
                module
                    .and_then(text)
                    .and_then(|name| last_segment(&name, &["."])),
            );
        }
        _ => {}
    }
}

/// One element of a PowerShell command: a parameter (`-Name`, lowercased,
/// without its `-` and a trailing `:`), or an argument's values (a bare
/// token; a static quoted literal, or each of a list of them, without
/// quotes).
enum PowerShellElement {
    Parameter(String),
    Argument(Vec<String>),
}

/// `Import-Module`'s parameters that take no value.
const IMPORT_MODULE_SWITCHES: &[&str] = &[
    "ascustomobject",
    "disablenamechecking",
    "force",
    "global",
    "noclobber",
    "passthru",
    "skipeditioncheck",
    "usewindowspowershell",
];

/// One command element: `None` for a separator or a redirection, which take
/// no position; any operand that is not a static literal (`$prefix`,
/// `(Get-X)`, `"$name"`, `"$(Get-X)"`) is an argument without values, so it
/// still takes its position and a preceding parameter's value (001 T008
/// review R4, R8).
fn powershell_element(element: tree_sitter::Node, source: &[u8]) -> Option<PowerShellElement> {
    let parameter = |text: &str| {
        PowerShellElement::Parameter(
            text.trim_start_matches('-')
                .trim_end_matches(':')
                .to_ascii_lowercase(),
        )
    };
    match element.kind() {
        "command_argument_sep" => None,
        kind if kind.contains("redirection") => None,
        "command_parameter" => Some(parameter(text(element, source)?)),
        "generic_token" => {
            let token = text(element, source)?;
            Some(if token.starts_with('-') {
                parameter(token)
            } else if token.contains('$') {
                // A bare token may embed a variable (`Mods/$name`).
                PowerShellElement::Argument(Vec::new())
            } else {
                PowerShellElement::Argument(vec![token.to_owned()])
            })
        }
        // `'./Store.psm1'`, `"Store"`, `'A', 'B'`: static string literals,
        // each inside a unary expression of an array literal; anything else
        // in the list, a string holding a variable or a subexpression
        // (`"$name"`, `"$(Get-X)"`) included, makes the whole operand
        // unknown (001 T008 review R8).
        "array_literal_expression" => {
            let expands = |literal: &tree_sitter::Node| {
                named_children(*literal).any(|string| {
                    matches!(
                        string.kind(),
                        "expandable_string_literal" | "expandable_here_string_literal"
                    ) && string.named_child_count() > 0
                })
            };
            let mut literals = Vec::new();
            for item in named_children(element) {
                let Some(literal) = named_child_of(item, &["string_literal"])
                    .filter(|literal| item.named_child_count() == 1 && !expands(literal))
                    .and_then(|literal| text(literal, source))
                else {
                    return Some(PowerShellElement::Argument(Vec::new()));
                };
                let inner = literal
                    .strip_prefix(['\'', '"'])
                    .and_then(|rest| rest.strip_suffix(['\'', '"']));
                literals.push(inner.unwrap_or(literal).to_owned());
            }
            Some(PowerShellElement::Argument(literals))
        }
        _ => Some(PowerShellElement::Argument(Vec::new())),
    }
}

/// The last non-empty piece of `path` split at each of `separators`.
fn last_segment(path: &str, separators: &[&str]) -> Option<String> {
    let mut last = path;
    for separator in separators {
        if let Some((_, after)) = last.rsplit_once(separator) {
            last = after;
        }
    }
    let last = last.trim();
    (!last.is_empty()).then(|| last.to_owned())
}

/// A Dart URI's file stem: `package:a/b.dart` gives `b`, `dart:async`
/// gives `async`.
fn dart_stem(uri: &str) -> Option<String> {
    let uri = uri.trim_matches(['\'', '"']);
    let uri = uri.rsplit_once(':').map_or(uri, |(_, path)| path);
    file_stem(uri)
}

/// The bound names of one Rust `use` argument: a path's last segment, an
/// alias, each member of a list, and `self` in a list as its path's last
/// segment; a glob gives none. Walked on the heap, like the unit walk.
fn rust_use_keys(argument: tree_sitter::Node, source: &[u8], out: &mut Vec<String>) {
    let text = |node: tree_sitter::Node| node.utf8_text(source).ok().map(str::to_owned);
    let last_segment = |path: tree_sitter::Node| match path.kind() {
        "identifier" => text(path),
        "scoped_identifier" => path
            .child_by_field_name("name")
            .filter(|name| name.kind() == "identifier")
            .and_then(text),
        _ => None,
    };
    // Each node with the path of the list that holds it, if any.
    let mut stack = vec![(argument, None)];
    while let Some((node, list_path)) = stack.pop() {
        match node.kind() {
            "identifier" | "scoped_identifier" => out.extend(last_segment(node)),
            "use_as_clause" => out.extend(node.child_by_field_name("alias").and_then(text)),
            "self" => out.extend(list_path.and_then(last_segment)),
            "scoped_use_list" => {
                if let Some(list) = node.child_by_field_name("list") {
                    stack.push((list, node.child_by_field_name("path")));
                }
            }
            "use_list" => {
                let mut cursor = node.walk();
                let members: Vec<_> = node.named_children(&mut cursor).collect();
                // Reversed onto the stack, so members are taken in order.
                stack.extend(members.into_iter().rev().map(|member| (member, list_path)));
            }
            _ => {}
        }
    }
}

fn last_named_child(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    node.named_child(u32::try_from(node.named_child_count().checked_sub(1)?).ok()?)
}

/// The file stem of a quoted or bracketed path (`"x/y.h"`, `<sys/types.h>`,
/// `'./tools/index.js'`): its last component without its extension.
fn file_stem(quoted: &str) -> Option<String> {
    let path = quoted.trim_matches(|c| matches!(c, '"' | '\'' | '`' | '<' | '>'));
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = without_extension(name);
    (!stem.is_empty()).then(|| stem.to_owned())
}
