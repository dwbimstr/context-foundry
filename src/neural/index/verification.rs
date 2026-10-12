//! Bounded, disposable receipts for immutable ANN files. A receipt retains the
//! verified inode and checks its version AND the currently opened name. It is
//! an I/O optimization for the trusted local writer, not file authentication.
use super::*;
use std::fs::File;
use std::os::unix::fs::MetadataExt;
use std::sync::Arc;
#[cfg(feature = "semantic")]
use std::sync::{Mutex, MutexGuard, PoisonError, TryLockError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp(u64, u64, u64, i64, i64, i64, i64);
impl Stamp {
    fn read(file: &File) -> Result<Self, GenerationError> {
        let m = file.metadata().map_err(|e| unavailable(e.to_string()))?;
        Ok(Self(
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        ))
    }
}

#[derive(Debug)]
pub(crate) struct ManifestReceipt {
    file: File,
    stamp: Stamp,
    #[cfg(feature = "semantic")]
    directory: (u64, u64),
}
impl ManifestReceipt {
    pub(super) fn capture(file: &File, _directory: &Dir) -> Result<Arc<Self>, GenerationError> {
        Ok(Arc::new(Self {
            stamp: Stamp::read(file)?,
            file: file.try_clone().map_err(|e| unavailable(e.to_string()))?,
            #[cfg(feature = "semantic")]
            directory: _directory
                .identity()
                .map_err(|e| unavailable(e.to_string()))?,
        }))
    }
    pub(super) fn unchanged(&self) -> bool {
        Stamp::read(&self.file).is_ok_and(|s| s == self.stamp)
    }
    #[cfg(feature = "semantic")]
    pub(crate) fn current(&self, store: &Dir, digest: &str) -> bool {
        let Ok(Some(dir)) = open_generation_dir(store, digest, false) else {
            return false;
        };
        dir.identity().is_ok_and(|id| id == self.directory)
            && dir
                .open_file(MANIFEST_FILE)
                .is_ok_and(|f| Stamp::read(&f).is_ok_and(|s| s == self.stamp))
            && self.unchanged()
    }
}

struct Receipt {
    file: File,
    stamp: Stamp,
    #[cfg(feature = "semantic")]
    resident: Option<Arc<usearch::Index>>,
}

/// Verified immutable metadata, retained once per artifact role.
pub(super) struct Metadata<T> {
    hash: String,
    file: File,
    stamp: Stamp,
    value: T,
}
pub(super) fn metadata<T: Clone>(
    slot: &mut Option<Metadata<T>>,
    mut file: File,
    hash: &str,
    limit: u64,
    control: &Control,
    decode: impl FnOnce(&[u8]) -> Result<T, GenerationError>,
) -> Result<T, GenerationError> {
    control.check().map_err(GenerationError::Interrupted)?;
    let stamp = Stamp::read(&file)?;
    if stamp.2 > limit {
        return Err(unavailable("metadata exceeds its byte bound"));
    }
    if let Some(old) = slot.as_ref()
        && old.hash == hash
        && old.stamp == stamp
        && Stamp::read(&old.file).is_ok_and(|now| now == stamp)
    {
        return Ok(old.value.clone());
    }
    *slot = None;
    neural_fault!(METADATA_READ, Some(control), hash).map_err(GenerationError::Interrupted)?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| unavailable(e.to_string()))?;
    if bytes.len() as u64 > limit {
        return Err(unavailable("metadata exceeds its byte bound"));
    }
    if crate::digest(&bytes) != hash {
        return Err(unavailable("metadata does not match the manifest hash"));
    }
    if Stamp::read(&file)? != stamp {
        return Err(unavailable("metadata changed during verification"));
    }
    let value = decode(&bytes)?;
    control.check().map_err(GenerationError::Interrupted)?;
    *slot = Some(Metadata {
        hash: hash.into(),
        file,
        stamp,
        value: value.clone(),
    });
    Ok(value)
}

/// At most the two segments named by the latest validated manifest. Old queries
/// own their immutable Arc separately; no cache entry can grow a delta chain.
#[derive(Default)]
pub(crate) struct Cache {
    directory: Option<(u64, u64)>,
    recipe: Option<String>,
    pub(super) members: Option<Metadata<Arc<Vec<String>>>>,
    pub(super) locations: Option<Metadata<Locations>>,
    pub(super) overlay: Option<Metadata<locations::Overlay>>,
    pub(super) outside: Option<(String, String, std::collections::BTreeSet<String>)>,
    pub(super) snapshot: Option<(String, Locations)>,
    receipts: std::collections::BTreeMap<String, Receipt>,
}
impl Cache {
    pub(super) fn prepare(
        &mut self,
        dir: &Dir,
        manifest: &GenerationManifest,
    ) -> Result<(), GenerationError> {
        let directory = dir.identity().map_err(|e| unavailable(e.to_string()))?;
        if self.directory != Some(directory) || self.recipe.as_deref() != Some(&manifest.recipe_id)
        {
            self.receipts.clear();
            self.members = None;
            self.locations = None;
            self.overlay = None;
            self.outside = None;
            self.snapshot = None;
            self.directory = Some(directory);
            self.recipe = Some(manifest.recipe_id.clone());
        }
        self.receipts.retain(|hash, _| {
            hash == &manifest.index_sha256
                || manifest.delta.as_ref().is_some_and(|d| hash == &d.sha256)
        });
        Ok(())
    }
    pub(super) fn verify(
        &mut self,
        mut file: File,
        hash: &str,
        limit: u64,
        keep: bool,
        control: &Control,
    ) -> Result<Option<Segment>, GenerationError> {
        control.check().map_err(GenerationError::Interrupted)?;
        let before = Stamp::read(&file)?;
        if before.2 > limit {
            return Err(unavailable("index file exceeds its byte bound"));
        }
        let valid = self
            .receipts
            .get(hash)
            .is_some_and(|r| r.stamp == before && Stamp::read(&r.file).is_ok_and(|s| s == before));
        if valid {
            if !keep {
                return Ok(None);
            }
            #[cfg(feature = "semantic")]
            if let Some(index) = self.receipts.get(hash).and_then(|r| r.resident.clone()) {
                return Ok(Some(Segment::Resident(index)));
            }
        } else {
            self.receipts.remove(hash);
        }
        let held = file.try_clone().map_err(|e| unavailable(e.to_string()))?;
        if !valid {
            neural_fault!(SEGMENT_HASH, Some(control), hash)
                .map_err(GenerationError::Interrupted)?;
        }
        let bytes = if keep {
            let (bytes, digest) = read_index(file, control, limit, !valid)?;
            if digest.as_deref().is_some_and(|digest| digest != hash) {
                return Err(unavailable("index file does not match the manifest hash"));
            }
            Some(Segment::Bytes(bytes))
        } else {
            if hash_reader(&mut file, control, limit)? != hash {
                return Err(unavailable("index file does not match the manifest hash"));
            }
            None
        };
        if Stamp::read(&held)? != before {
            self.receipts.remove(hash);
            return Err(unavailable("index file changed during verification"));
        }
        self.receipts.insert(
            hash.to_owned(),
            Receipt {
                file: held,
                stamp: before,
                #[cfg(feature = "semantic")]
                resident: None,
            },
        );
        Ok(bytes)
    }
    #[cfg(feature = "semantic")]
    pub(crate) fn remember_locations(
        &mut self,
        directory: (u64, u64),
        id: String,
        locations: Locations,
    ) {
        if self.directory != Some(directory) {
            *self = Self::default();
            self.directory = Some(directory);
        }
        self.snapshot = Some((id, locations));
    }
    #[cfg(feature = "semantic")]
    pub(crate) fn remember(&mut self, hash: &str, index: Arc<usearch::Index>) {
        if let Some(receipt) = self.receipts.get_mut(hash) {
            receipt.resident = Some(index);
        }
    }
}

/// Publication verifies away from the owner slot. Query waits remain subject to
/// their deadline if that verification currently owns the shared receipt cache.
#[cfg(feature = "semantic")]
pub(crate) fn lock<'a>(
    cache: &'a Mutex<Cache>,
    control: &Control,
) -> Result<MutexGuard<'a, Cache>, GenerationError> {
    loop {
        control.check().map_err(GenerationError::Interrupted)?;
        match cache.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(error)) => return Ok(PoisonError::into_inner(error)),
            Err(TryLockError::WouldBlock) => {
                std::thread::sleep(std::time::Duration::from_millis(2))
            }
        }
    }
}

#[cfg(all(test, feature = "semantic", feature = "test-faults"))]
mod tests {
    use super::*;
    use crate::fault::{self, Action};
    use crate::neural::fault_names::SEGMENT_HASH;
    use std::time::{Duration, Instant};

    #[test]
    fn metadata_receipts_obey_bounds_cancellation_and_file_replacement() {
        use crate::neural::fault_names::METADATA_READ;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("metadata");
        let bytes = b"[1,2,3]";
        let hash = crate::digest(bytes);
        std::fs::write(&path, bytes).unwrap();
        let mut slot = None;
        let read = |slot: &mut Option<Metadata<Vec<u8>>>, limit, control: &Control| {
            metadata(
                slot,
                File::open(&path).unwrap(),
                &hash,
                limit,
                control,
                |b| Ok(b.to_vec()),
            )
        };
        assert!(read(&mut slot, 64, &Control::cancelled()).is_err());
        assert!(slot.is_none());
        let target = path.clone();
        fault::arm(
            METADATA_READ,
            0,
            Action::Call(Box::new(move |_| {
                std::fs::write(&target, bytes).unwrap();
            })),
        );
        assert!(read(&mut slot, 64, &Control::unbounded()).is_err());
        assert!(slot.is_none());
        fault::disarm_all();
        assert_eq!(read(&mut slot, 64, &Control::unbounded()).unwrap(), bytes);
        assert_eq!(fault::reached(METADATA_READ), 1);
        assert_eq!(read(&mut slot, 64, &Control::unbounded()).unwrap(), bytes);
        assert_eq!(
            fault::reached(METADATA_READ),
            1,
            "unchanged metadata is not reread"
        );
        assert!(
            read(&mut slot, 2, &Control::unbounded()).is_err(),
            "limits apply even to receipts"
        );
        let replacement = tmp.path().join("new");
        std::fs::write(&replacement, b"[4,5,6]").unwrap();
        std::fs::rename(&replacement, &path).unwrap();
        assert!(read(&mut slot, 64, &Control::unbounded()).is_err());
        assert!(
            slot.is_none(),
            "substituted metadata cannot inherit a receipt"
        );
    }

    #[test]
    fn cancellation_and_change_during_read_never_mint_a_receipt() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("segment");
        let bytes = vec![7u8; 2048];
        let hash = crate::digest(&bytes);
        std::fs::write(&path, &bytes).unwrap();
        let mut cache = Cache::default();
        assert!(matches!(
            cache.verify(
                File::open(&path).unwrap(),
                &hash,
                4096,
                true,
                &Control::cancelled()
            ),
            Err(GenerationError::Interrupted(_))
        ));
        assert!(cache.receipts.is_empty());
        let target = path.clone();
        fault::arm(
            SEGMENT_HASH,
            0,
            Action::Call(Box::new(move |_| {
                // Even identical bytes rewritten in place invalidate the pre-read stamp.
                std::fs::write(&target, vec![7u8; 2048]).unwrap();
            })),
        );
        assert!(
            cache
                .verify(
                    File::open(&path).unwrap(),
                    &hash,
                    4096,
                    true,
                    &Control::unbounded()
                )
                .is_err()
        );
        assert!(cache.receipts.is_empty());
        fault::disarm_all();
        cache
            .verify(
                File::open(&path).unwrap(),
                &hash,
                4096,
                true,
                &Control::unbounded(),
            )
            .unwrap();
        assert_eq!(cache.receipts.len(), 1);
    }

    #[test]
    fn receipt_cache_wait_obeys_control_and_membership_eviction_is_bounded() {
        let cache = Mutex::new(Cache::default());
        let held = cache.lock().unwrap();
        let control = Control::with_deadline(Instant::now() + Duration::from_millis(5));
        assert!(matches!(
            lock(&cache, &control),
            Err(GenerationError::Interrupted(_))
        ));
        drop(held);
        let tmp = tempfile::tempdir().unwrap();
        let dir = Dir::open_path(tmp.path()).unwrap();
        let mut cache = cache.into_inner().unwrap();
        let mut manifest = GenerationManifest {
            v: GENERATION_VERSION,
            function_digest: "ab".repeat(32),
            recipe_id: "test".into(),
            dimensions: 128,
            metric: METRIC.into(),
            scalar: SCALAR_KIND.into(),
            count: 0,
            index_sha256: String::new(),
            base_entries: Arc::new(vec![]),
            base_count: 0,
            members_sha256: "ab".repeat(32),
            delta: None,
            labels_sha256: "cd".repeat(32),
            locations_delta: None,
            mapping_revision: None,
            coverage_stats: None,
            source_revision: 0,
            coverage: COVERAGE_PARTIAL.into(),
        };
        for i in 0..10 {
            let bytes = vec![i; 16];
            let hash = crate::digest(&bytes);
            let name = format!("segment-{i}");
            dir.write_new(&name, &bytes).unwrap();
            if i == 0 {
                manifest.index_sha256 = hash.clone();
            } else {
                manifest.delta = Some(DeltaManifest {
                    sha256: hash.clone(),
                    entries: vec![],
                });
            }
            cache.prepare(&dir, &manifest).unwrap();
            cache
                .verify(
                    dir.open_file(&name).unwrap(),
                    &hash,
                    4096,
                    false,
                    &Control::unbounded(),
                )
                .unwrap();
            assert_eq!(cache.receipts.len(), if i == 0 { 1 } else { 2 });
            assert!(cache.receipts.contains_key(&manifest.index_sha256));
        }
    }
}
