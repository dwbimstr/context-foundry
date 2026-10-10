//! Syntax corroborates a compiler-resolved occurrence; it never resolves names.
use crate::{SourceHandle, graph::ReferenceItem};
use tree_sitter::Node;

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

fn target(mut n: Node<'_>) -> Node<'_> {
    loop {
        let field = match n.kind() {
            "generic_function" => "function",
            "scoped_identifier" => "name",
            "field_expression" => "field",
            "attribute" => "attribute",
            _ => return n,
        };
        match n.child_by_field_name(field) {
            Some(next) => n = next,
            None => return n,
        }
    }
}

pub(crate) fn classify(tree: &tree_sitter::Tree, body: &str, r: &ReferenceItem) -> Use {
    let mut result = Use::default();
    let Some(mut node) = tree
        .root_node()
        .descendant_for_byte_range(r.start as usize, r.end as usize)
    else {
        return result;
    };
    let handle = |start: usize, end: usize| {
        (end > start && end - start <= super::MAX_SPAN_BYTES as usize).then(|| SourceHandle {
            start: start as u64,
            end: end as u64,
            ..r.unit.clone()
        })
    };
    loop {
        if matches!(node.kind(), "call_expression" | "call")
            && let Some(function) = node.child_by_field_name("function")
        {
            let name = target(function);
            if name.start_byte() as u64 == r.start && name.end_byte() as u64 == r.end {
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

#[cfg(test)]
mod tests {
    use super::*;
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
