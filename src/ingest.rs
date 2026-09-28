use crate::Engine;
use anyhow::{Context, Result};
use serde::Serialize;
use std::{collections::BTreeSet, io::Read, path::Path};

#[derive(Default, Debug, Serialize)]
pub struct SyncReport {
    pub changed: usize,
    pub unchanged: usize,
    pub deleted: usize,
    pub skipped: Vec<String>,
    pub failures: Vec<String>,
    pub deletions_deferred: bool,
}

pub fn sync(engine: &mut Engine, root: &Path) -> Result<SyncReport> {
    let root = root.canonicalize()?;
    anyhow::ensure!(root != engine.directory, "cannot index the store directory");
    engine.bind_workspace(&root)?;
    let mut report = SyncReport::default();
    let mut present = BTreeSet::new();
    let data_directory = engine.directory.clone();
    let walker = ignore::WalkBuilder::new(&root)
        .hidden(true)
        .follow_links(false)
        .filter_entry(move |entry| {
            entry.path() != data_directory
                && !matches!(
                    entry.file_name().to_str(),
                    Some("target" | "node_modules" | ".context-foundry")
                )
        })
        .build();
    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                report.failures.push(e.to_string());
                continue;
            }
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(&root)?
            .to_str()
            .context("source path is not UTF-8")?
            .to_owned();
        present.insert(rel.clone());
        if matches!(
            entry.file_name().to_str(),
            Some(".env" | "id_rsa" | "id_ed25519")
        ) {
            engine.delete_source(&rel)?;
            report.skipped.push(format!("{rel}: sensitive filename"));
            continue;
        }
        let read = (|| -> Result<Vec<u8>> {
            let mut bytes = Vec::new();
            std::fs::File::open(entry.path())?
                .take(2 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            Ok(bytes)
        })();
        match read {
            Ok(bytes) => {
                let excluded = if bytes.len() > 2 * 1024 * 1024 {
                    Some("over 2 MiB")
                } else if bytes.contains(&0) {
                    Some("binary NUL")
                } else if std::str::from_utf8(&bytes).is_err() {
                    Some("not UTF-8")
                } else {
                    None
                };
                if let Some(reason) = excluded {
                    report.deleted += usize::from(engine.delete_source(&rel)?);
                    report.skipped.push(format!("{rel}: {reason}"));
                    continue;
                }
                let content = String::from_utf8(bytes)?;
                // A narrow deny rule, not a claim that arbitrary prose secrets can be detected.
                if content.contains("-----BEGIN PRIVATE KEY-----")
                    || content.contains("-----BEGIN RSA PRIVATE KEY-----")
                {
                    engine.delete_source(&rel)?;
                    report.skipped.push(format!("{rel}: private key marker"));
                    continue;
                }
                if engine.replace_source(&rel, &content)? {
                    report.changed += 1;
                } else {
                    report.unchanged += 1;
                }
            }
            Err(error) => report.failures.push(format!("{rel}: {error}")),
        }
    }
    // An incomplete enumeration cannot establish absence. Preserve the last accepted state.
    if report.failures.is_empty() {
        for path in engine.paths()? {
            if !present.contains(&path) && engine.delete_source(&path)? {
                report.deleted += 1;
            }
        }
    } else {
        report.deletions_deferred = true;
    }
    while engine.refresh_index()? != 0 {}
    Ok(report)
}
