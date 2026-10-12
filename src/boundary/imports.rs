//! Narrow, source-only Python imports for cross-file router mounts and named
//! re-exports. Cycles, shadowing, escape and unconfigured modules stay incomplete.
//! All files are explicitly configured providers of the same service. This is
//! not Python import execution or a claim about runtime module resolution.
use super::*;

#[derive(Default)]
pub(super) struct External {
    pub mounts: BTreeMap<String, (String, Vec<Span>)>,
    pub accepted: BTreeSet<usize>,
    pub incomplete: bool,
    pub reexport_only: bool,
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
    if (rest.is_empty() && dots == 0)
        || (!rest.is_empty()
            && rest.split('.').any(|p| {
                p.is_empty() || !p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            }))
    {
        return None;
    }
    if !rest.is_empty() {
        parts.extend(rest.split('.'));
    }
    let stem = parts.join("/");
    let file = format!("{stem}.py");
    let package = format!("{stem}/__init__.py");
    // Exactly one explicitly configured module or package. Search-path
    // precedence and competing layouts are not guessed.
    match (admitted.contains(&file), admitted.contains(&package)) {
        (true, false) => Some(file),
        (false, true) => Some(package),
        _ => None,
    }
}

pub(super) fn collect(
    service: &Service,
    bodies: &BTreeMap<String, (SourceMeta, String)>,
    control: &Control,
) -> FResult<BTreeMap<String, External>> {
    let mut out = BTreeMap::<String, External>::new();
    let mut claimed = BTreeSet::new();
    let mut edges = BTreeMap::<String, BTreeSet<String>>::new();
    let mut exports = BTreeMap::<String, BTreeMap<String, (String, String, Span)>>::new();
    let mut mounts = Vec::new();
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
        let import_starts: BTreeSet<_> =
            bindings.values().map(|(_, _, n)| n.start_byte()).collect();
        // A pure forwarding module need not construct its own app. Arbitrary
        // module code, unsupported imports or conditional exports cannot use
        // this exemption from the extractor's missing-app check.
        out.entry(path.clone()).or_default().reexport_only = !bindings.is_empty()
            && top.iter().all(|n| {
                import_starts.contains(&n.start_byte())
                    || n.kind() == "comment"
                    || (n.kind() == "expression_statement"
                        && n.named_child(0)
                            .is_some_and(|n| literal(n, source).is_some()))
            });
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
            mounts.push((
                path.clone(),
                target.clone(),
                symbol.clone(),
                prefix,
                vec![span(path, &meta.hash, *import), span(path, &meta.hash, n)],
            ));
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
        exports.insert(
            path.clone(),
            bindings
                .into_iter()
                .map(|(alias, (target, symbol, import))| {
                    (alias, (target, symbol, span(path, &meta.hash, import)))
                })
                .collect(),
        );
    }
    // Resolve only explicit named imports after every configured module has
    // been examined. The original binding/escape checks still govern each
    // intermediate module. No source lookup or Python execution occurs here.
    for (importer, mut target, mut symbol, prefix, mut dependencies) in mounts {
        let mut visited = BTreeSet::new();
        loop {
            control.check()?;
            if !visited.insert(target.clone()) || visited.len() > service.providers.len() {
                out.entry(importer.clone()).or_default().incomplete = true;
                break;
            }
            if let Some((next, name, import)) = exports.get(&target).and_then(|e| e.get(&symbol)) {
                dependencies.push(import.clone());
                target = next.clone();
                symbol = name.clone();
                continue;
            }
            let destination = out.entry(target.clone()).or_default();
            if claimed.insert((target.clone(), symbol.clone())) {
                destination.mounts.insert(symbol, (prefix, dependencies));
            } else {
                destination.mounts.remove(&symbol);
                destination.incomplete = true;
            }
            break;
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
