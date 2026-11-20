//! Bounded input files: open one operator-named regular file without
//! following a link or blocking on a FIFO, and read a whole file under a
//! byte cap that is checked before and during the read.
use super::*;

// ---------------------------------------------------------------------------
// Bounded input files
// ---------------------------------------------------------------------------

/// Open one operator-named input file (policy, tokenizer) for reading
/// without following a final-component symlink or blocking on a FIFO;
/// anything but a regular file is refused.
pub(crate) fn open_regular(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    Ok(file)
}

/// Read a whole regular file of at most `limit` bytes: the descriptor's own
/// length is checked BEFORE anything is read, and the read is capped at
/// `limit + 1` bytes so a file that grows meanwhile is still refused.
/// `None` when the file is over the limit.
pub(crate) fn read_capped(file: File, limit: u64) -> std::io::Result<Option<Vec<u8>>> {
    if file.metadata()?.len() > limit {
        return Ok(None);
    }
    let mut body = Vec::new();
    file.take(limit + 1).read_to_end(&mut body)?;
    Ok((body.len() as u64 <= limit).then_some(body))
}
