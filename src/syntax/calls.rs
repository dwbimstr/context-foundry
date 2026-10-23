//! Calls: the languages whose call names are indexed, call targets and names,
//! test assertion calls, and Python import aliases, shared with the task
//! evidence readers. Unit extraction stays in the parent module.
use super::*;

/// Supported call-name syntax. Other language parsers retain ordinary indexing.
pub(super) fn call_language(lang: Lang) -> bool {
    matches!(
        lang,
        Lang::Rust | Lang::Python | Lang::Go | Lang::TypeScript | Lang::Tsx | Lang::JavaScript
    )
}

pub(crate) fn call_target(mut n: tree_sitter::Node<'_>) -> tree_sitter::Node<'_> {
    loop {
        let field = match n.kind() {
            "generic_function" => "function",
            "scoped_identifier" => "name",
            "field_expression" => "field",
            "attribute" => "attribute",
            "member_expression" => "property",
            "selector_expression" => "field",
            _ => return n,
        };
        match n.child_by_field_name(field) {
            Some(next) => n = next,
            None => return n,
        }
    }
}

/// The Rust assertion expression macros whose arguments are searched for calls.
const ASSERTION_MACROS: [&str; 6] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
];

/// `arguments` is a parenthesized token tree after `name`, with only
/// whitespace between them.
fn parenthesized_after(
    name: tree_sitter::Node<'_>,
    arguments: tree_sitter::Node<'_>,
    body: &str,
) -> bool {
    arguments.kind() == "token_tree"
        && body[arguments.byte_range()].starts_with('(')
        && body[name.end_byte()..arguments.start_byte()]
            .trim()
            .is_empty()
}

/// Rust assertion macros keep their arguments as token trees. A name directly
/// followed by parentheses is a useful unresolved candidate in these expression
/// macros; strings, values passed as arguments and arbitrary macro DSLs are not.
pub(crate) fn assertion_call(node: tree_sitter::Node<'_>, body: &str) -> Option<(usize, usize)> {
    if node.kind() != "identifier" {
        return None;
    }
    let arguments = node.next_named_sibling()?;
    if !parenthesized_after(node, arguments, body) {
        return None;
    }
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if ancestor.kind() == "macro_invocation" {
            let name = ancestor.child_by_field_name("macro")?;
            return ASSERTION_MACROS
                .contains(&&body[name.byte_range()])
                .then_some((node.start_byte(), arguments.end_byte()));
        }
        if ancestor.kind() != "token_tree" {
            return None;
        }
        parent = ancestor.parent();
    }
    None
}

/// Use the ingestion walk's existing ancestry and previous sibling. Node's
/// parent/sibling helpers can re-descend from the root; calling them per token
/// would make deeply nested ingestion unnecessarily expensive. Nested macro
/// token trees beyond this recipe's fixed depth remain visibly incomplete.
pub(super) fn indexed_assertion_call<'b>(
    arguments: tree_sitter::Node<'_>,
    previous: Option<tree_sitter::Node<'_>>,
    ancestors: &[tree_sitter::Node<'_>],
    body: &'b str,
) -> (Option<&'b str>, bool) {
    let Some(name) = previous.filter(|node| node.kind() == "identifier") else {
        return (None, false);
    };
    if !parenthesized_after(name, arguments, body) {
        return (None, false);
    }
    for ancestor in ancestors.iter().rev().take(32) {
        if ancestor.kind() == "macro_invocation" {
            let supported = ancestor
                .child_by_field_name("macro")
                .is_some_and(|node| ASSERTION_MACROS.contains(&&body[node.byte_range()]));
            return (supported.then(|| &body[name.byte_range()]), false);
        }
        if ancestor.kind() != "token_tree" {
            return (None, false);
        }
    }
    (None, ancestors.len() >= 32)
}

pub(crate) fn call_name<'t>(
    node: tree_sitter::Node<'t>,
    body: &str,
) -> Option<tree_sitter::Node<'t>> {
    if matches!(node.kind(), "call_expression" | "call") {
        node.child_by_field_name("function").map(call_target)
    } else {
        assertion_call(node, body).map(|_| node)
    }
}

/// A deliberately conservative Python alias recipe, shared by ingestion and
/// final-snapshot candidate discovery. This does not resolve Python bindings.
#[derive(Clone, Debug)]
pub(crate) struct PythonImportAlias {
    pub name: String,
    pub declaration: (usize, usize),
}

#[derive(Default)]
pub(super) struct PythonImportAliases {
    aliases: std::collections::BTreeMap<String, Option<PythonImportAlias>>,
    declarations: std::collections::BTreeSet<(usize, usize)>,
    other_names: std::collections::BTreeSet<String>,
    calls: std::collections::BTreeMap<String, usize>,
    limited: bool,
}

impl PythonImportAliases {
    pub(super) fn observe(
        &mut self,
        node: tree_sitter::Node,
        ancestors: &[tree_sitter::Node],
        body: &str,
    ) {
        if self.limited {
            return;
        }
        let parent = ancestors.last().copied();
        if node.kind() == "wildcard_import" {
            // A star import can introduce a conflicting name without spelling it.
            self.limited = true;
            return;
        }
        if node.kind() == "aliased_import"
            && let Some(import) = parent.filter(|p| p.kind() == "import_from_statement")
            && ancestors
                .iter()
                .rev()
                .nth(1)
                .is_some_and(|p| p.kind() == "module")
            && let Some(name) = node.child_by_field_name("name")
            && let Some(alias) = node.child_by_field_name("alias")
        {
            let original = &body[name.byte_range()];
            let bound = &body[alias.byte_range()];
            if original.contains('.') || original == bound {
                return;
            }
            if self.declarations.len() == 128
                || original.len() > 1024
                || bound.len() > 1024
                || !original.is_ascii()
                || !bound.is_ascii()
            {
                self.limited = true;
                return;
            }
            self.declarations
                .insert((alias.start_byte(), alias.end_byte()));
            self.aliases
                .entry(bound.to_owned())
                .and_modify(|old| *old = None)
                .or_insert_with(|| {
                    Some(PythonImportAlias {
                        name: original.to_owned(),
                        declaration: (import.start_byte(), import.end_byte()),
                    })
                });
        }
        if node.kind() != "identifier"
            || self
                .declarations
                .contains(&(node.start_byte(), node.end_byte()))
        {
            return;
        }
        let name = &body[node.byte_range()];
        if name.len() > 1024 || !name.is_ascii() {
            // Python applies NFKC to identifiers. Refuse this narrow recipe
            // rather than miss an equivalent Unicode shadowing spelling.
            self.limited = true;
            return;
        }
        let direct_call = parent
            .is_some_and(|p| p.kind() == "call" && p.child_by_field_name("function") == Some(node));
        if direct_call {
            // The last call is kept: `finish` requires it after the import.
            if let Some(at) = self.calls.get_mut(name) {
                *at = node.start_byte();
            } else if self.calls.len() == 4096 {
                self.limited = true;
            } else {
                self.calls.insert(name.to_owned(), node.start_byte());
            }
        } else if !self.other_names.contains(name) {
            if self.other_names.len() == 4096 {
                self.limited = true;
            } else {
                // Any other use refuses the alias for the entire source. This
                // catches every explicit binding role, including parameters,
                // comprehensions, assignments and nested imports, at the cost of
                // refusing harmless ordinary value references and attribute names.
                self.other_names.insert(name.to_owned());
            }
        }
    }

    pub(super) fn finish(self) -> (std::collections::BTreeMap<String, PythonImportAlias>, bool) {
        if self.limited {
            return (Default::default(), true);
        }
        let aliases = self
            .aliases
            .into_iter()
            .filter_map(|(bound, alias)| {
                let alias = alias?;
                (!self.other_names.contains(&bound)
                    && self
                        .calls
                        .get(&bound)
                        .is_some_and(|at| *at >= alias.declaration.1))
                .then_some((bound, alias))
            })
            .collect();
        (aliases, false)
    }
}

pub(crate) fn python_import_aliases(
    tree: &tree_sitter::Tree,
    body: &str,
    control: &crate::Control,
) -> crate::FResult<(std::collections::BTreeMap<String, PythonImportAlias>, bool)> {
    let mut aliases = PythonImportAliases::default();
    let mut cursor = tree.walk();
    let mut ancestors = Vec::new();
    let mut visited = 0usize;
    loop {
        visited += 1;
        if visited.is_multiple_of(256) {
            control.check()?;
        }
        let node = cursor.node();
        aliases.observe(node, &ancestors, body);
        if cursor.goto_first_child() {
            ancestors.push(node);
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return Ok(aliases.finish());
            }
            ancestors.pop();
        }
    }
}
