//! Direct authoritative reads (context-v2 contract order): `Engine::retrieve`,
//! `Engine::retrieve_outline` and the verified handle read they share. Handle
//! parsing, freshness and the token-budget check stay in the parent module.
use super::*;

impl Engine {
    /// Direct authoritative read in context-v2 contract order: syntax and
    /// bounds (handle and `lines`), `ws16` prefix of the bound `workspace_id`,
    /// existence, `sha32` prefix of the stored SHA-256, then range — the last
    /// three against one final read transaction. With `lines`, the outcome's
    /// handle names the intersection of those file lines with the handle's
    /// range. The outcome carries the full stored identities. Never touches
    /// the derived index.
    pub fn retrieve(
        &self,
        handle: &str,
        lines: Option<&str>,
        tokens: usize,
    ) -> FResult<RetrieveOutcome> {
        check_token_budget(tokens)?;
        let parsed = HandleRef::parse(handle)?;
        let lines = lines.map(LineSelection::parse).transpose()?;
        let bound = self.require_workspace_id()?;
        if !bound.starts_with(&parsed.ws16) {
            return Err(FoundryError::WrongWorkspace);
        }
        let read =
            self.read_handle_span(&parsed.path, parsed.start, parsed.end, &parsed.sha32, lines)?;
        let requested = SourceHandle {
            workspace_id: bound,
            path: parsed.path,
            sha256: read.sha256.clone(),
            start: read.start,
            end: read.end,
        };
        Ok(read.outcome(requested, tokens))
    }

    /// `view:"outline"` (context-v2 § Retrieve views): the requested range,
    /// after any `lines` clipping, in the `outline` and `outline-min` forms,
    /// validated in contract order like [`Self::retrieve`]. An unmapped
    /// language is `unsupported_mode`; a mapped source over 1 MiB is not
    /// parsed, so its outline equals its text.
    pub fn retrieve_outline(
        &self,
        handle: &str,
        lines: Option<&str>,
        tokens: usize,
    ) -> FResult<OutlineOutcome> {
        check_token_budget(tokens)?;
        let parsed = HandleRef::parse(handle)?;
        let lines = lines.map(LineSelection::parse).transpose()?;
        let bound = self.require_workspace_id()?;
        if !bound.starts_with(&parsed.ws16) {
            return Err(FoundryError::WrongWorkspace);
        }
        let read =
            self.read_handle_span(&parsed.path, parsed.start, parsed.end, &parsed.sha32, lines)?;
        let Some(lang) = crate::syntax::Lang::from_path(&parsed.path) else {
            return Err(FoundryError::UnsupportedMode(
                "view:\"outline\" needs a mapped language".into(),
            ));
        };
        let outliner = crate::syntax::Outliner::new(&read.body, lang);
        let range = read.start as usize..read.end as usize;
        let span = &read.body.as_bytes()[range.clone()];
        let end_line = read.start_line
            + span[..span.len().saturating_sub(1)]
                .iter()
                .filter(|&&b| b == b'\n')
                .count() as u64;
        let outline = outliner.render(range.clone(), 60, 120);
        let outline_min = outliner.render(range, 0, 0);
        Ok(OutlineOutcome {
            requested: SourceHandle {
                workspace_id: bound,
                path: parsed.path,
                sha256: read.sha256,
                start: read.start,
                end: read.end,
            },
            start_line: read.start_line,
            end_line,
            lang: lang.tag(),
            outline,
            outline_min,
            freshness: read.freshness,
        })
    }

    /// Existence, digest (`sha32` against the prefix of the stored full
    /// SHA-256) and range, then the optional `lines` intersection, in one final
    /// read transaction.
    fn read_handle_span(
        &self,
        path: &str,
        start: u64,
        end: u64,
        sha32: &str,
        lines: Option<LineSelection>,
    ) -> FResult<HandleRead> {
        // Boundary after validation, before the final authoritative read.
        fault!(RETRIEVE_BEFORE_FINAL_READ, Some(self), None, path)?;
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let Some(raw) = sources.get(path)? else {
            return Err(FoundryError::NotFound);
        };
        let meta: SourceMeta = decode(raw.value(), "source")?;
        if !meta.hash.starts_with(sha32) {
            return Err(FoundryError::StaleHandle);
        }
        let verified = reconstruct_verified(&stored, path, &meta)?;
        let body = &verified.body;
        let len = body.len() as u64;
        let valid_empty = start == 0 && end == 0 && meta.bytes == 0;
        let valid_span = start < end
            && end <= len
            && body.is_char_boundary(start as usize)
            && body.is_char_boundary(end as usize);
        if !valid_empty && !valid_span {
            return Err(FoundryError::InvalidRange);
        }
        let (start, end) = match lines {
            None => (start as usize, end as usize),
            Some(lines) => lines.clip(body.as_bytes(), start as usize, end as usize)?,
        };
        let start_line = body.as_bytes()[..start]
            .iter()
            .filter(|&&b| b == b'\n')
            .count() as u64
            + 1;
        let freshness = self.freshness_in(&tx)?;
        Ok(HandleRead {
            sha256: meta.hash,
            start: start as u64,
            end: end as u64,
            start_line,
            source_bytes: len,
            freshness,
            body: verified.body,
        })
    }
}

/// One validated handle read: the verified source, the read's range within it
/// and the stored full digest.
struct HandleRead {
    sha256: String,
    start: u64,
    end: u64,
    start_line: u64,
    source_bytes: u64,
    freshness: Freshness,
    /// The whole verified source. `start..end` was validated against it, so
    /// slicing its byte view yields the read span.
    body: String,
}

impl HandleRead {
    fn outcome(self, requested: SourceHandle, requested_tokens: usize) -> RetrieveOutcome {
        RetrieveOutcome {
            requested,
            requested_tokens,
            span: self.body.as_bytes()[self.start as usize..self.end as usize].to_vec(),
            start_line: self.start_line,
            source_bytes: self.source_bytes,
            freshness: self.freshness,
        }
    }
}
