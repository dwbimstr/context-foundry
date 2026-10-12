//! Literal Flask and directly registered Blueprint routes. Deliberately excludes
//! dynamic factories, nested blueprints, converter plugins and escaped bindings.
use super::*;

fn prefix(call: Node, source: &str, skip: usize) -> Option<Option<String>> {
    let args = children(call.child_by_field_name("arguments")?);
    if !(skip..=skip + 1).contains(&args.len()) {
        return None;
    }
    let Some(arg) = args.get(skip) else {
        return Some(None);
    };
    if arg.kind() != "keyword_argument"
        || text(arg.child_by_field_name("name")?, source) != "url_prefix"
    {
        return None;
    }
    let value = literal(arg.child_by_field_name("value")?, source)?;
    (value.is_empty()
        || value.starts_with('/') && !value.ends_with('/') && !value.contains(['<', '>', '{', '}']))
    .then_some(Some(value))
}
fn route(value: &str) -> Option<String> {
    if value.contains(['{', '}']) {
        return None;
    }
    let mut result = Vec::new();
    for part in value.split('/') {
        if !part.contains(['<', '>']) {
            result.push(part.to_owned());
            continue;
        }
        let inside = part.strip_prefix('<')?.strip_suffix('>')?;
        let (kind, name) = inside.split_once(':').unwrap_or(("string", inside));
        let kind = match kind {
            "string" => "str",
            "int" => "int",
            _ => return None,
        };
        result.push(format!("{{{name}:{kind}}}"));
    }
    let result = result.join("/");
    route_path(&result).then_some(result)
}

pub(super) fn extract(
    config: &Config,
    path: &str,
    hash: &str,
    source: &str,
    root: Node,
    nodes: &[Node],
) -> Option<(Vec<Fact>, bool)> {
    let service = config
        .services
        .iter()
        .find(|s| s.providers.iter().any(|p| p == path))?;
    let mut constructors = BTreeMap::new();
    let mut approved = BTreeSet::new();
    for n in children(root) {
        let Some(items) = text(n, source).strip_prefix("from flask import ") else {
            continue;
        };
        for item in items.split(',') {
            let words: Vec<_> = item.split_whitespace().collect();
            let (kind, alias) = match words.as_slice() {
                [kind] if ["Flask", "Blueprint"].contains(kind) => (*kind, *kind),
                [kind, "as", alias] if ["Flask", "Blueprint"].contains(kind) => (*kind, *alias),
                _ => continue,
            };
            constructors.insert(alias.to_owned(), kind.to_owned());
            approved.insert((n.start_byte(), n.end_byte()));
        }
    }
    if constructors.is_empty() {
        return None;
    }
    let mut bindings = BTreeMap::new();
    let mut constructed = BTreeSet::new();
    let mut complete = true;
    for &n in nodes {
        if n.kind() != "assignment"
            || n.parent()
                .and_then(|p| p.parent())
                .is_none_or(|p| p.kind() != "module")
        {
            continue;
        }
        let (Some(left), Some(right)) = (
            n.child_by_field_name("left"),
            n.child_by_field_name("right"),
        ) else {
            continue;
        };
        let Some(f) = right.child_by_field_name("function") else {
            continue;
        };
        let Some(kind) = constructors.get(text(f, source)) else {
            continue;
        };
        let args = right
            .child_by_field_name("arguments")
            .map(children)
            .unwrap_or_default();
        let skip = if kind == "Flask" { 1 } else { 2 };
        let valid = left.kind() == "identifier"
            && args.len() >= skip
            && text(args[skip - 1], source) == "__name__"
            && (skip == 1 || literal(args[0], source).is_some());
        let own_prefix = if kind == "Flask" {
            (args.len() == 1).then_some(None)
        } else {
            prefix(right, source, skip)
        };
        if !valid || own_prefix.is_none() {
            complete = false;
            continue;
        }
        if bindings
            .insert(
                text(left, source).to_owned(),
                (kind == "Flask", own_prefix.flatten().unwrap_or_default(), n),
            )
            .is_some()
        {
            complete = false;
        }
        approved.insert((left.start_byte(), left.end_byte()));
        constructed.insert((right.start_byte(), right.end_byte()));
    }
    let mut mounts = BTreeMap::new();
    for &n in nodes {
        if n.kind() != "call" {
            continue;
        }
        if n.child_by_field_name("function")
            .is_some_and(|f| constructors.contains_key(text(f, source)))
            && !constructed.contains(&(n.start_byte(), n.end_byte()))
        {
            complete = false;
        }
        let Some((receiver, method)) = n
            .child_by_field_name("function")
            .and_then(|f| attr(f, source))
        else {
            continue;
        };
        if method != "register_blueprint" || !bindings.contains_key(&receiver) {
            continue;
        }
        let parsed = (|| {
            if !bindings[&receiver].0
                || bindings[&receiver].2.start_byte() > n.start_byte()
                || n.parent()
                    .and_then(|p| p.parent())
                    .is_none_or(|p| p.kind() != "module")
            {
                return None;
            }
            let arg = first_argument(n)?;
            let name = text(arg, source);
            let binding = bindings.get(name)?;
            if arg.kind() != "identifier" || binding.0 || binding.2.start_byte() > n.start_byte() {
                return None;
            }
            let value = prefix(n, source, 1)?.unwrap_or_else(|| binding.1.clone());
            approved.insert((arg.start_byte(), arg.end_byte()));
            Some((name.to_owned(), (value, n, receiver.clone())))
        })();
        if let Some((name, mount)) = parsed {
            if mounts.insert(name, mount).is_some() {
                complete = false;
            }
        } else {
            complete = false;
        }
    }
    // Reject escaping/shadowing of constructors and app/blueprint bindings. An
    // unrecognized mutation makes the provider scope incomplete below.
    let mut unsafe_binding = false;
    for &n in nodes {
        if n.kind() != "identifier"
            || !(constructors.contains_key(text(n, source))
                || bindings.contains_key(text(n, source)))
        {
            continue;
        }
        if approved
            .iter()
            .any(|&(lo, hi)| n.start_byte() >= lo && n.end_byte() <= hi)
        {
            continue;
        }
        let safe = n.parent().is_some_and(|p| {
            (constructors.contains_key(text(n, source))
                && p.kind() == "call"
                && p.child_by_field_name("function") == Some(n))
                || (bindings.contains_key(text(n, source))
                    && p.kind() == "attribute"
                    && p.child_by_field_name("object") == Some(n)
                    && p.parent().is_some_and(|c| {
                        c.kind() == "call" && c.child_by_field_name("function") == Some(p)
                    }))
        });
        unsafe_binding |= !safe;
    }
    complete &= !unsafe_binding && !bindings.is_empty();
    complete &= bindings
        .iter()
        .all(|(name, (app, _, _))| *app || mounts.contains_key(name));
    let mut facts = Vec::new();
    // Flask(__name__) also registers its default static route. The constructor
    // is evidence of that declaration, not proof that any static file exists.
    if !unsafe_binding {
        for (_, _, decl) in bindings.values().filter(|(app, _, _)| *app) {
            for method in ["GET", "HEAD", "OPTIONS"] {
                facts.push(Fact {
                    role: "provide".into(),
                    key: Some(Key {
                        service: service.id.clone(),
                        method: method.into(),
                        path: "/static/{filename:flask_path}".into(),
                    }),
                    evidence: span(path, hash, *decl),
                    context: span(path, hash, *decl),
                    dependencies: vec![],
                    unsupported: None,
                });
            }
        }
    }
    for &n in nodes {
        if n.kind() != "call" {
            continue;
        }
        let Some((receiver, method)) = n
            .child_by_field_name("function")
            .and_then(|f| attr(f, source))
        else {
            continue;
        };
        let Some((app, own, decl)) = bindings.get(&receiver) else {
            continue;
        };
        if method == "register_blueprint" {
            continue;
        }
        let parsed = (|| {
            if unsafe_binding
                || n.start_byte() < decl.start_byte()
                || n.parent().is_none_or(|p| p.kind() != "decorator")
                || n.parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                    .is_none_or(|p| p.kind() != "module")
            {
                return None;
            }
            let args = children(n.child_by_field_name("arguments")?);
            let value = literal(*args.first()?, source)?;
            let mut methods = Vec::new();
            if method == "route" {
                match args.len() {
                    1 => methods.push("GET".to_owned()),
                    2 => {
                        let arg = args[1];
                        if arg.kind() != "keyword_argument"
                            || text(arg.child_by_field_name("name")?, source) != "methods"
                        {
                            return None;
                        }
                        let list = arg.child_by_field_name("value")?;
                        if !["list", "tuple"].contains(&list.kind()) {
                            return None;
                        }
                        for item in children(list) {
                            methods.push(literal(item, source)?.to_ascii_uppercase());
                        }
                    }
                    _ => return None,
                }
            } else if ["get", "post", "put", "patch", "delete"].contains(&method.as_str())
                && args.len() == 1
            {
                methods.push(method.to_ascii_uppercase());
            } else {
                return None;
            }
            if methods.is_empty()
                || methods.len() > 8
                || methods.iter().any(|m| {
                    !["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"]
                        .contains(&m.as_str())
                })
            {
                return None;
            }
            if methods.iter().any(|m| m == "GET") {
                methods.push("HEAD".into());
            }
            methods.push("OPTIONS".into());
            methods.sort();
            methods.dedup();
            let mut dependencies = vec![span(path, hash, *decl)];
            let prefix = if *app {
                own.clone()
            } else {
                let (prefix, mount, owner) = mounts.get(&receiver)?;
                if mount.start_byte() < n.start_byte() {
                    return None;
                }
                dependencies.push(span(path, hash, *mount));
                dependencies.push(span(path, hash, bindings[owner].2));
                prefix.clone()
            };
            let route = route(&format!("{prefix}{value}"))?;
            Some((route, methods, dependencies))
        })();
        let Some((route, methods, dependencies)) = parsed else {
            complete = false;
            continue;
        };
        for method in methods {
            if facts.len() == MAX_FACTS {
                complete = false;
                break;
            }
            facts.push(Fact {
                role: "provide".into(),
                key: Some(Key {
                    service: service.id.clone(),
                    method,
                    path: route.clone(),
                }),
                evidence: span(path, hash, n),
                context: span(path, hash, n.parent().and_then(|p| p.parent()).unwrap_or(n)),
                dependencies: dependencies.clone(),
                unsupported: None,
            });
        }
    }
    Some((facts, complete))
}
