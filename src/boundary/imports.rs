//! Narrow, source-only Python imports for direct cross-file router mounts.
//! All files are explicitly configured providers of the same service. This is
//! not Python import execution or a claim about runtime module resolution.
use super::*;

#[derive(Default)]
pub(super) struct External {
    pub mounts: BTreeMap<String, (String, Vec<Span>)>,
    pub accepted: BTreeSet<usize>,
    pub incomplete: bool,
    pub inputs: BTreeMap<String, Option<String>>,
}

fn module_path(path: &str, module: &str, admitted: &[String]) -> Option<String> {
    let dots = module.bytes().take_while(|b| *b == b'.').count();
    let mut parts: Vec<&str> = if dots == 0 {
        vec![]
    } else {
        let mut p: Vec<_> = path.split('/').collect();
        p.pop();
        if dots > p.len() {
            return None; // Relative import without an admitted containing package.
        }
        for _ in 1..dots {
            p.pop()?;
        }
        p
    };
    let rest = &module[dots..];
    if rest.is_empty()
        || rest
            .split('.')
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
    {
        return None;
    }
    parts.extend(rest.split('.'));
    let stem = parts.join("/");
    let file = format!("{stem}.py");
    // Package initializers/re-exports and competing module layouts are outside
    // this recipe. The configured namespace is the only search path.
    (admitted.contains(&file) && !admitted.contains(&format!("{stem}/__init__.py"))).then_some(file)
}

pub(super) fn collect(
    service: &Service,
    bodies: &BTreeMap<String, (SourceMeta, String)>,
    control: &Control,
) -> FResult<BTreeMap<String, External>> {
    let mut out = BTreeMap::<String, External>::new();
    let mut claimed = BTreeSet::new();
    let mut edges = BTreeMap::<String, BTreeSet<String>>::new();
    let mut active = false;
    for path in &service.providers {
        control.check()?;
        let Some((meta, source)) = bodies.get(path) else {
            continue;
        };
        if source.len() > crate::syntax::MAX_PARSE_BYTES {
            continue;
        }
        let Some(tree) = crate::syntax::parse_boundary_python(source).ok().flatten() else {
            continue;
        };
        let root = tree.root_node();
        if root.has_error() {
            continue;
        }
        let top = children(root);
        let apps: BTreeSet<_> = top
            .iter()
            .filter_map(|statement| {
                let assignment = statement.named_child(0)?;
                if assignment.kind() != "assignment" {
                    return None;
                }
                let left = assignment.child_by_field_name("left")?;
                let right = assignment.child_by_field_name("right")?;
                (left.kind() == "identifier" && text(right, source) == "FastAPI()")
                    .then(|| text(left, source).to_owned())
            })
            .collect();
        let mut bindings = BTreeMap::new();
        for &n in &top {
            let t = text(n, source);
            let words: Vec<_> = t.split_whitespace().collect();
            let (module, symbol, alias) = match words.as_slice() {
                ["from", module, "import", symbol] => (*module, *symbol, *symbol),
                ["from", module, "import", symbol, "as", alias] => (*module, *symbol, *alias),
                _ => continue,
            };
            let Some(target) = module_path(path, module, &service.providers) else {
                continue;
            };
            edges
                .entry(path.clone())
                .or_default()
                .insert(target.clone());
            if ![symbol, alias]
                .iter()
                .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
            {
                continue;
            }
            active = true;
            if bindings
                .insert(alias.to_owned(), (target, symbol.to_owned(), n))
                .is_some()
            {
                out.entry(path.clone()).or_default().incomplete = true;
            }
        }
        let mut approved: BTreeMap<String, Vec<(usize, usize)>> = bindings
            .iter()
            .map(|(name, (_, _, n))| (name.clone(), vec![(n.start_byte(), n.end_byte())]))
            .collect();
        for &statement in &top {
            if statement.kind() != "expression_statement" {
                continue;
            }
            let Some(n) = statement.named_child(0).filter(|n| n.kind() == "call") else {
                continue;
            };
            let Some((receiver, method)) = n
                .child_by_field_name("function")
                .and_then(|f| attr(f, source))
            else {
                continue;
            };
            if method != "include_router" || !apps.contains(&receiver) {
                continue;
            }
            let Some(arg) = first_argument(n).filter(|a| a.kind() == "identifier") else {
                continue;
            };
            let Some((target, symbol, import)) = bindings.get(text(arg, source)) else {
                continue;
            };
            let Some(prefix) = static_prefix(n, source, 1) else {
                out.entry(path.clone()).or_default().incomplete = true;
                continue;
            };
            // The main extractor independently checks that the receiver really
            // is an admitted app. Unsupported receiver/mutation keeps its scope
            // incomplete, so this hint cannot certify a unique service join.
            if import.start_byte() > n.start_byte() || target == path {
                out.entry(path.clone()).or_default().incomplete = true;
                continue;
            }
            approved
                .get_mut(text(arg, source))
                .unwrap()
                .push((arg.start_byte(), arg.end_byte()));
            out.entry(path.clone())
                .or_default()
                .accepted
                .insert(n.start_byte());
            let destination = out.entry(target.clone()).or_default();
            if claimed.insert((target.clone(), symbol.clone())) {
                destination.mounts.insert(
                    symbol.clone(),
                    (
                        prefix,
                        vec![span(path, &meta.hash, *import), span(path, &meta.hash, n)],
                    ),
                );
            } else {
                destination.mounts.remove(symbol);
                destination.incomplete = true;
            }
        }
        // Any escaped/shadowed imported alias makes the entire service proof
        // incomplete. Do not follow alias assignments, calls or star imports.
        let mut stack = vec![root];
        let mut visited = 0;
        while let Some(n) = stack.pop() {
            visited += 1;
            if visited > 100_000 {
                out.entry(path.clone()).or_default().incomplete = true;
                break;
            }
            if visited % 256 == 0 {
                control.check()?;
            }
            if n.kind() == "identifier"
                && let Some(ranges) = approved.get(text(n, source))
                && !ranges
                    .iter()
                    .any(|&(start, end)| start <= n.start_byte() && n.end_byte() <= end)
            {
                out.entry(path.clone()).or_default().incomplete = true;
            }
            stack.extend(children(n));
        }
    }
    if active {
        let cyclic = edges.keys().any(|start| {
            let mut visited = BTreeSet::new();
            let mut stack: Vec<_> = edges[start].iter().collect();
            while let Some(node) = stack.pop() {
                if node == start {
                    return true;
                }
                if visited.insert(node)
                    && let Some(next) = edges.get(node)
                {
                    stack.extend(next);
                }
            }
            false
        });
        let inputs: BTreeMap<_, _> = service
            .providers
            .iter()
            .map(|p| (p.clone(), bodies.get(p).map(|(m, _)| m.hash.clone())))
            .collect();
        for path in &service.providers {
            out.entry(path.clone()).or_default().inputs = inputs.clone();
            out.entry(path.clone()).or_default().incomplete |= cyclic;
        }
    }
    Ok(out)
}
