//! Unit extraction: the tree walk that turns a parsed source into unit
//! candidates, with the statement-form, member-signature and leading-comment
//! checks only it uses. The per-language unit rules, names and bodies stay in
//! the parent module.
use super::*;

/// The unit candidates of one parse; for an outline, also the comment and
/// declaration-only member ranges it needs, and for indexing the import keys.
/// A source over a [`ScannerLimit`] is stopped before the parser runs.
pub(super) fn tree_units(
    source: &str,
    lang: Lang,
    need: Need,
    extras: &mut Analysis,
) -> Vec<Candidate> {
    let mut out = Vec::new();
    let Some(grammar) = lang.grammar() else {
        return out;
    };
    let bytes = source.as_bytes();
    if let Some(stopped) = over_scanner_limit(lang, bytes) {
        extras.stopped = Some(stopped);
        return out;
    }
    let tree = match parse(source, &grammar) {
        Ok(Some(tree)) => tree,
        Ok(None) => return out,
        Err(stopped) => {
            extras.stopped = Some(stopped);
            return out;
        }
    };
    extras.clean_tree = !tree.root_node().has_error();
    let collect_calls = need == Need::Imports && call_language(lang) && extras.clean_tree;
    if need == Need::Imports && call_language(lang) {
        extras.call_coverage = Some(if collect_calls {
            "complete"
        } else {
            "incomplete"
        });
    }
    // Iterative pre-order walk: error-recovered trees can be deep. The
    // ancestors of the current node are kept on the heap, because
    // `Node::parent` (and so `prev_sibling`) re-descends from the root. Per
    // depth, `runs` holds the consecutive leading-run nodes (§ Unit forest)
    // just before the current node at that depth, and `facts` what the
    // ancestor at that depth says about its children ([`Facts`]), read once.
    let mut cursor = tree.walk();
    let mut ancestors: Vec<tree_sitter::Node> = Vec::new();
    let mut facts: Vec<Facts> = Vec::new();
    let mut runs: Vec<Vec<(tree_sitter::Node, Leading)>> = vec![Vec::new()];
    let mut previous_named = vec![None];
    let mut python_aliases = PythonImportAliases::default();
    'walk: loop {
        let node = cursor.node();
        // Units, members and imports are named nodes; keyword tokens such
        // as F#'s `namespace` or Haskell's `import` share their kind names.
        if node.is_named() {
            if collect_calls && lang == Lang::Python {
                python_aliases.observe(node, &ancestors, source);
            }
            // A Rust declaration-only item is a unit and stays a mandatory
            // outline member, so outlines keep its lines as before.
            let at = Walk {
                lang,
                ancestors: &ancestors,
                facts: &facts,
                source: bytes,
            };
            candidates(&at, node, &runs, need, &mut out);
            if need == Need::Imports {
                import_keys(lang, node, bytes, &mut extras.imports);
                if collect_calls {
                    let (name, incomplete) = if matches!(node.kind(), "call_expression" | "call") {
                        (
                            call_name(node, source)
                                .map(|name| source[name.byte_range()].to_owned()),
                            false,
                        )
                    } else if lang == Lang::Rust && node.kind() == "token_tree" {
                        indexed_assertion_call(
                            node,
                            *previous_named.last().unwrap(),
                            &ancestors,
                            source,
                        )
                    } else {
                        (None, false)
                    };
                    if incomplete {
                        extras.call_coverage = Some("incomplete");
                    }
                    if let Some(name) = name {
                        // One name per source, on its first search document.
                        if name.len() > 1024
                            || (extras.call_names.len() == 4096
                                && !extras.call_names.contains(&name))
                        {
                            extras.call_coverage = Some("incomplete");
                        } else {
                            extras.call_names.insert(name);
                        }
                    }
                }
            } else if need == Need::Outline {
                if is_member_signature(&at, node) {
                    // A template wrapper's lines belong to the signature too.
                    let outer = at.outermost_wrapper(node);
                    extras.members.push((outer.start_byte(), outer.end_byte()));
                } else if node.kind().ends_with("comment")
                    && node.end_position().row + 1 >= node.start_position().row + COMMENT_LINES
                {
                    extras.comments.push((node.start_byte(), node.end_byte()));
                }
            }
            *previous_named.last_mut().unwrap() = Some(node);
        }
        if cursor.goto_first_child() {
            previous_named.push(None);
            facts.push(Facts::of(lang, node));
            ancestors.push(node);
            runs.push(Vec::new());
            continue;
        }
        loop {
            // The current node's subtree is done: it now precedes its next sibling.
            let finished = cursor.node();
            let run = runs.last_mut().expect("one run per depth");
            match leading_kind(lang, finished, bytes) {
                Some(kind) => run.push((finished, kind)),
                None => run.clear(),
            }
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                break 'walk;
            }
            ancestors.pop();
            facts.pop();
            runs.pop();
            previous_named.pop();
        }
    }
    close_statement_forms(bytes, &mut out);
    if collect_calls && lang == Lang::Python {
        let (aliases, limited) = python_aliases.finish();
        if limited {
            extras.call_coverage = Some("incomplete");
        }
        for alias in aliases.into_values() {
            if extras.call_names.len() == 4096 && !extras.call_names.contains(&alias.name) {
                extras.call_coverage = Some("incomplete");
            } else {
                extras.call_names.insert(alias.name);
            }
        }
    }
    out
}

/// Extends each statement-form container (see [`Open`]) over its members:
/// to the next container's start in the same parent, or (chained, or the
/// last) to the parent's end, without the whitespace before that point
/// unless a member's range covers it (Perl's last block runs over the
/// file's trailing whitespace and comments). The bytes after its own
/// statement are its body.
fn close_statement_forms(source: &[u8], candidates: &mut [Candidate]) {
    let mut open: Vec<(usize, usize, usize)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, c)| c.open.map(|open| (open.parent, c.start, index)))
        .collect();
    if open.is_empty() {
        return;
    }
    open.sort_unstable();
    let mut spans: Vec<(usize, usize)> = candidates.iter().map(|c| (c.start, c.end)).collect();
    spans.sort_unstable();
    // Latest member end of the next chained container of the same parent,
    // whose window lies inside this one's; walking backwards reads each
    // member once.
    let mut carried = 0;
    for (position, &(parent, start, index)) in open.iter().enumerate().rev() {
        let Some(Open {
            parent_end, form, ..
        }) = candidates[index].open
        else {
            continue;
        };
        let next = open
            .get(position + 1)
            .filter(|next| next.0 == parent)
            .map(|next| next.1);
        let (end, inherited) = match form {
            Form::Block => {
                carried = 0;
                continue;
            }
            Form::Statement => (next.unwrap_or(parent_end), 0),
            Form::Chained => (parent_end, if next.is_some() { carried } else { 0 }),
        };
        let own = candidates[index].end;
        let mut end = end.max(own);
        let members = spans.partition_point(|span| span.0 < start)
            ..spans.partition_point(|span| span.0 < next.unwrap_or(end).min(end));
        let reach = spans[members]
            .iter()
            .map(|span| span.1)
            .max()
            .unwrap_or(0)
            .max(inherited);
        carried = reach;
        let floor = reach.clamp(own, end);
        while end > floor && source[end - 1].is_ascii_whitespace() {
            end -= 1;
        }
        candidates[index].end = end;
        candidates[index].body = (own < end).then_some((own, end));
    }
}

/// A declaration-only member (a trait method without a body, an interface,
/// protocol or abstract member, a C/C++ prototype, a Haskell or F#
/// signature): not a unit, yet a signature that an outline never hides.
fn is_member_signature(at: &Walk, node: tree_sitter::Node) -> bool {
    let (lang, source) = (at.lang, at.source);
    let above = |n: usize| at.above(n);
    let bodiless = || node.child_by_field_name("body").is_none();
    match lang {
        Lang::Rust => matches!(node.kind(), "function_signature_item" | "associated_type"),
        Lang::Go => node.kind() == "method_elem",
        Lang::TypeScript | Lang::Tsx => matches!(
            node.kind(),
            "method_signature"
                | "property_signature"
                | "abstract_method_signature"
                | "call_signature"
                | "construct_signature"
                | "index_signature"
        ),
        Lang::C | Lang::Cpp => {
            matches!(node.kind(), "declaration" | "field_declaration") && declares_function(node)
        }
        Lang::CSharp => {
            above(1) == Some("declaration_list")
                && above(2) == Some("interface_declaration")
                && match node.kind() {
                    "method_declaration" => bodiless(),
                    kind => matches!(
                        kind,
                        "property_declaration" | "event_declaration" | "indexer_declaration"
                    ),
                }
        }
        Lang::FSharp => node.kind() == "member_signature",
        Lang::FSharpSignature => matches!(node.kind(), "member_signature" | "value_definition"),
        Lang::VbNet => {
            matches!(node.kind(), "method_declaration" | "property_declaration")
                && above(1) == Some("interface_block")
        }
        Lang::Php => node.kind() == "method_declaration" && bodiless(),
        Lang::Kotlin => {
            node.kind() == "function_declaration"
                && named_child_of(node, &["function_body"]).is_none()
                && matches!(above(1), Some("class_body" | "enum_class_body"))
        }
        Lang::Swift => matches!(
            node.kind(),
            "protocol_function_declaration" | "protocol_property_declaration"
        ),
        Lang::Scala => node.kind() == "function_declaration",
        Lang::Dart => {
            node.kind() == "declaration"
                && node.named_child(0).is_some_and(|first| {
                    matches!(
                        first.kind(),
                        "function_signature" | "getter_signature" | "setter_signature"
                    )
                })
        }
        Lang::Elixir => {
            node.kind() == "call"
                && elixir_definition(node, source).is_some_and(|(keyword, _)| {
                    matches!(keyword, ElixirDef::Function) && elixir_body(node, source).is_none()
                })
        }
        Lang::Haskell => node.kind() == "signature",
        _ => false,
    }
}

/// Whether one of a declaration's declarator chains holds a function
/// declarator (a prototype rather than a variable or field).
fn declares_function(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    node.children_by_field_name("declarator", &mut cursor)
        .any(|mut declarator| {
            loop {
                if declarator.kind() == "function_declarator" {
                    return true;
                }
                let inner = declarator.child_by_field_name("declarator").or_else(|| {
                    matches!(
                        declarator.kind(),
                        "reference_declarator" | "parenthesized_declarator"
                    )
                    .then(|| unnamed_declarator(declarator))
                    .flatten()
                });
                match inner {
                    Some(inner) => declarator = inner,
                    None => return false,
                }
            }
        })
}

fn leading_kind(lang: Lang, node: tree_sitter::Node, source: &[u8]) -> Option<Leading> {
    let doc_block = || {
        node.utf8_text(source)
            .is_ok_and(|text| text.starts_with("/**"))
    };
    match (lang, node.kind()) {
        (Lang::Rust, "attribute_item") => Some(Leading::Attribute),
        (Lang::Rust, "line_comment" | "block_comment") => {
            node.child_by_field_name("outer").map(|_| Leading::Doc)
        }
        (Lang::Java, "block_comment")
        | (Lang::TypeScript | Lang::Tsx | Lang::JavaScript, "comment") => {
            doc_block().then_some(Leading::Doc)
        }
        (Lang::Go, "comment") => Some(Leading::Doc),
        // A type's `<Attribute>` block precedes its declaration as a sibling
        // (a member's is inside it).
        (Lang::VbNet, "attribute_block") => Some(Leading::Attribute),
        _ => None,
    }
}
