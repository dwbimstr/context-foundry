//! Syntax corroborates compiler occurrences and discovers explicitly unresolved
//! call-name candidates. Neither operation resolves names or proves execution.
use crate::syntax::{assertion_call, call_name, call_target};
use crate::{SourceHandle, graph::ReferenceItem};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default)]
pub(crate) struct Use {
    pub call: Option<SourceHandle>,
    pub test: Option<SourceHandle>,
}

pub(crate) fn key(reference: &ReferenceItem) -> String {
    format!(
        "{}:{}",
        reference.unit.workspace_id, reference.occurrence_id
    )
}

pub(crate) fn classify(tree: &tree_sitter::Tree, body: &str, r: &ReferenceItem) -> Use {
    classify_at(
        tree,
        body,
        &SourceHandle {
            start: r.start,
            end: r.end,
            ..r.unit.clone()
        },
    )
}

fn classify_at(tree: &tree_sitter::Tree, body: &str, occurrence: &SourceHandle) -> Use {
    let mut result = Use::default();
    let Some(mut node) = tree
        .root_node()
        .descendant_for_byte_range(occurrence.start as usize, occurrence.end as usize)
    else {
        return result;
    };
    let handle = |start: usize, end: usize| {
        (end > start && end - start <= super::MAX_SPAN_BYTES as usize).then(|| SourceHandle {
            start: start as u64,
            end: end as u64,
            ..occurrence.clone()
        })
    };
    loop {
        if matches!(node.kind(), "call_expression" | "call")
            && let Some(function) = node.child_by_field_name("function")
        {
            let name = call_target(function);
            if name.start_byte() as u64 == occurrence.start
                && name.end_byte() as u64 == occurrence.end
            {
                result.call = handle(node.start_byte(), node.end_byte());
            }
        }
        if node.kind() == "function_item" {
            let mut previous = node.prev_named_sibling();
            let mut start = node.start_byte();
            let mut test = false;
            // Attributes are siblings in tree-sitter-rust. A cap prevents a
            // pathological attribute list from growing query work unboundedly.
            for _ in 0..32 {
                let Some(attribute) = previous else { break };
                if !matches!(
                    attribute.kind(),
                    "attribute_item" | "line_comment" | "block_comment"
                ) {
                    break;
                }
                if attribute.kind() == "attribute_item" {
                    let text = &body[attribute.byte_range()];
                    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                    test |= ["#[test]", "#[tokio::test]", "#[async_std::test]"]
                        .contains(&compact.as_str())
                        || compact.starts_with("#[tokio::test(")
                        || compact.starts_with("#[async_std::test(");
                    start = attribute.start_byte();
                }
                previous = attribute.prev_named_sibling();
            }
            if test {
                result.test = handle(start, node.end_byte());
            }
            // A function nested in a test is not itself a test body.
            break;
        }
        let Some(parent) = node.parent() else { break };
        node = parent;
    }
    result
}

/// Inspect one admitted, bounded source for calls whose terminal name matches
/// an explicitly resolved query anchor. This is candidate discovery only:
/// shadowing, dispatch, imports and overloads remain unresolved. No graph edge
/// is created. Only the known assertion expression macros extend call nodes;
/// arbitrary macro token trees, comments and strings do not.
pub(crate) fn candidates(
    tree: &tree_sitter::Tree,
    body: &str,
    source: &SourceHandle,
    names: &BTreeSet<String>,
    limit: usize,
    control: &crate::Control,
) -> crate::FResult<(Vec<(SourceHandle, bool)>, bool)> {
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    let mut cursor = tree.walk();
    let mut visited = 0usize;
    loop {
        visited += 1;
        if visited.is_multiple_of(256) {
            control.check()?;
        }
        let node = cursor.node();
        let assertion = assertion_call(node, body);
        let name = call_name(node, body);
        if let Some(name) = name
            && names.contains(&body[name.byte_range()])
        {
            let occurrence = SourceHandle {
                start: name.start_byte() as u64,
                end: name.end_byte() as u64,
                ..source.clone()
            };
            let mut usage = classify_at(tree, body, &occurrence);
            if let Some((start, end)) = assertion {
                usage.call = Some(SourceHandle {
                    start: start as u64,
                    end: end as u64,
                    ..source.clone()
                });
            }
            if let Some(call) = usage.call {
                let mut enclosing = node;
                while let Some(parent) = enclosing.parent() {
                    enclosing = parent;
                    if matches!(
                        enclosing.kind(),
                        "function_item"
                            | "function_definition"
                            | "function_declaration"
                            | "method_declaration"
                            | "method_definition"
                            | "arrow_function"
                    ) {
                        break;
                    }
                }
                let handle = usage.test.clone().unwrap_or_else(|| {
                    if enclosing.parent().is_some()
                        && enclosing.end_byte() - enclosing.start_byte()
                            <= super::MAX_SPAN_BYTES as usize
                    {
                        SourceHandle {
                            start: enclosing.start_byte() as u64,
                            end: enclosing.end_byte() as u64,
                            ..source.clone()
                        }
                    } else {
                        call
                    }
                });
                if seen.insert((handle.start, handle.end)) {
                    if found.len() == limit {
                        return Ok((found, true));
                    }
                    found.push((handle, usage.test.is_some()));
                }
            }
        }
        if cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return Ok((found, false));
            }
        }
    }
}

/// Attributed tests in an already selected source may exercise a public wrapper
/// rather than call the private implementation directly. Rank up to four by
/// distinct task words, without claiming a call or requirement-coverage edge.
pub(crate) fn related_tests(
    tree: &tree_sitter::Tree,
    body: &str,
    source: &SourceHandle,
    words: &BTreeSet<String>,
    control: &crate::Control,
) -> crate::FResult<(Vec<SourceHandle>, bool)> {
    let mut best = Vec::new();
    let mut limited = false;
    let mut cursor = tree.walk();
    let mut visited = 0usize;
    loop {
        visited += 1;
        if visited.is_multiple_of(256) {
            control.check()?;
        }
        let node = cursor.node();
        if node.kind() == "function_item"
            && let Some(name) = node.child_by_field_name("name")
        {
            let occurrence = SourceHandle {
                start: name.start_byte() as u64,
                end: name.end_byte() as u64,
                ..source.clone()
            };
            if let Some(test) = classify_at(tree, body, &occurrence).test {
                let text = &body[test.start as usize..test.end as usize];
                let score = crate::syntax::code_subtokens(text)
                    .into_iter()
                    .map(|(s, e)| text[s..e].to_lowercase())
                    .filter(|word| words.contains(word))
                    .collect::<BTreeSet<_>>()
                    .len();
                if score > 0 {
                    best.push((score, test));
                    best.sort_by_key(|(score, handle)| (std::cmp::Reverse(*score), handle.start));
                    limited |= best.len() > 4;
                    best.truncate(4);
                }
            }
        }
        if cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return Ok((
                    best.into_iter().map(|(_, handle)| handle).collect(),
                    limited,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn call_candidates_are_bounded_and_exclude_text_and_value_references() {
        for (path, body, expected) in [
            (
                "lib.rs",
                "fn invoke_target() {}\nfn caller() { pkg::invoke_target(); let f = invoke_target; consume(invoke_target); let s = \"invoke_target()\"; }\n#[test]\nfn verifies() { invoke_target(); }\n",
                2,
            ),
            (
                "api.py",
                "def invoke_target():\n    pass\ndef caller():\n    pkg.invoke_target()\n    f = invoke_target\n    consume(invoke_target)\n    s = 'invoke_target()'\n",
                1,
            ),
            (
                "api.ts",
                "function invoke_target() {}\nfunction caller() { pkg.invoke_target(); const f = invoke_target; consume(invoke_target); const s = 'invoke_target()'; }\n",
                1,
            ),
            (
                "api.go",
                "package api\nfunc invoke_target() {}\nfunc caller() { pkg.invoke_target(); f := invoke_target; consume(invoke_target) }\n",
                1,
            ),
        ] {
            let tree = crate::syntax::parse_reference_source(path, body).unwrap();
            let source = SourceHandle {
                workspace_id: "w".into(),
                path: path.into(),
                sha256: crate::digest(body.as_bytes()),
                start: 0,
                end: body.len() as u64,
            };
            let names = ["invoke_target".to_owned()].into();
            let (found, limited) = candidates(
                &tree,
                body,
                &source,
                &names,
                8,
                &crate::Control::unbounded(),
            )
            .unwrap();
            assert_eq!(found.len(), expected, "{path}");
            assert!(!limited);
            for (handle, test) in &found {
                assert!(
                    body[handle.start as usize..handle.end as usize].contains("invoke_target()")
                );
                assert_eq!(handle.sha256, source.sha256);
                if *test {
                    assert!(
                        body[handle.start as usize..handle.end as usize].starts_with("#[test]")
                    );
                }
            }
            let (bounded, limited) = candidates(
                &tree,
                body,
                &source,
                &names,
                0,
                &crate::Control::unbounded(),
            )
            .unwrap();
            assert!(bounded.is_empty() && limited);
        }
    }

    #[test]
    fn assertion_candidates_and_local_tests_keep_their_distinct_basis() {
        let body = "fn invoke_target() {}\n#[test]\nfn verifies_bytes() { assert_eq!(invoke_target(), ()); }\n#[test]\nfn checks_wrapper_bytes() { wrapper(); }\nfn text_only() { let s = \"invoke_target()\"; dsl!(invoke_target()); }\n";
        let tree = crate::syntax::parse_reference_source("lib.rs", body).unwrap();
        let source = SourceHandle {
            workspace_id: "w".into(),
            path: "lib.rs".into(),
            sha256: crate::digest(body.as_bytes()),
            start: 0,
            end: body.len() as u64,
        };
        let (calls, limited) = candidates(
            &tree,
            body,
            &source,
            &["invoke_target".to_owned()].into(),
            8,
            &crate::Control::unbounded(),
        )
        .unwrap();
        assert!(!limited);
        assert_eq!(calls.len(), 1);
        assert!(calls[0].1);
        let (tests, limited) = related_tests(
            &tree,
            body,
            &source,
            &["bytes".to_owned()].into(),
            &crate::Control::unbounded(),
        )
        .unwrap();
        assert!(!limited);
        assert_eq!(tests.len(), 2);
        assert!(body[tests[1].start as usize..tests[1].end as usize].contains("wrapper()"));
        assert!(!body[tests[1].start as usize..tests[1].end as usize].contains("invoke_target"));
        assert!(
            related_tests(
                &tree,
                body,
                &source,
                &["absent".to_owned()].into(),
                &crate::Control::unbounded()
            )
            .unwrap()
            .0
            .is_empty()
        );
        assert!(crate::syntax::parse_reference_source("broken.rs", "fn (}").is_none());
    }

    fn usage(body: &str, needle: &str) -> Use {
        usage_in("lib.rs", body, needle)
    }
    fn usage_in(path: &str, body: &str, needle: &str) -> Use {
        let start = body.find(needle).unwrap();
        let r = ReferenceItem {
            occurrence_id: "id".into(),
            path: path.into(),
            sha256: crate::digest(body.as_bytes()),
            start: start as u64,
            end: (start + "target".len()) as u64,
            line: 1,
            unit: SourceHandle {
                workspace_id: "w".into(),
                path: path.into(),
                sha256: crate::digest(body.as_bytes()),
                start: 0,
                end: body.len() as u64,
            },
            label: "fn caller".into(),
            edge_id: Some("e".into()),
        };
        classify(
            &crate::syntax::parse_reference_source(path, body).unwrap(),
            body,
            &r,
        )
    }
    #[test]
    fn distinguishes_calls_arguments_pointer_bindings_and_test_attributes() {
        let body =
            "#[test]\nfn check() { crate::target::<u32>(); let f = target; invoke(target); }";
        let called = usage(body, "target::<");
        assert!(called.call.is_some() && called.test.is_some());
        assert_eq!(called.test.unwrap().start, 0);
        assert!(usage(body, "target;").call.is_none());
        assert!(usage(body, "target);").call.is_none());
        assert!(
            usage("fn test_named_only() { target(); }", "target")
                .test
                .is_none()
        );
        assert!(
            usage("#[test]\nfn check() { fn inner() { target(); } }", "target")
                .test
                .is_none()
        );
        assert!(
            usage(
                "#[tokio::test(flavor = \"current_thread\")]\nasync fn check() { target().await; }",
                "target"
            )
            .test
            .is_some()
        );
    }

    #[test]
    fn python_requires_the_resolved_occurrence_to_be_the_callee() {
        let called = usage_in(
            "api.py",
            "def caller():\n    return pkg.target(42)\n",
            "target",
        );
        assert!(called.call.is_some() && called.test.is_none());
        for body in ["invoke(target)\n", "target.method()\n", "f = target\n"] {
            assert!(usage_in("api.py", body, "target").call.is_none());
        }
        assert!(
            usage_in(
                "api.py",
                "async def test_named():\n    await target()\n",
                "target"
            )
            .call
            .is_some()
        );
    }
}
