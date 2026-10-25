//! Immutable location base plus one bounded replacement/tombstone overlay.
//! Persistent imbl trees share unchanged nodes across publication and serving;
//! all durable bytes remain ordinary, content-bound generation artifacts. The
//! base is a v6 label map ([`LabelTable`]); the overlay keeps whole locations.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub type Locations = imbl::OrdMap<String, Arc<Vec<UnitLocation>>>;
pub(crate) type Overlay = BTreeMap<String, Option<Arc<Vec<UnitLocation>>>>;
pub const MAX_CHANGES: usize = 4096;
pub const MAX_DELTA_BYTES: u64 = 32 * 1024 * 1024;

/// Generation v6 label map. `files` lists each distinct `[path, source
/// sha256]` pair once, sorted; `labels` lists each input key once, sorted,
/// with its locations as `[file, kind, start, end]`. A key's position is its
/// label ordinal, so a generation is deterministic for one cache state and no
/// unit identity ever comes from a storage ordinal. v5 repeated both strings in
/// every location: 191.4 MB for rust-lang/rust's 685,623 hierarchy locations.
#[cfg(feature = "semantic")]
pub(crate) struct LabelTable<'a> {
    files: Vec<(&'a str, &'a str)>,
    rows: Vec<(&'a str, &'a [UnitLocation])>,
}

#[cfg(feature = "semantic")]
impl<'a> LabelTable<'a> {
    /// [`Self::encode`] writes `rows` in the order given; publication passes
    /// them in ascending key order.
    pub(crate) fn new(rows: impl IntoIterator<Item = (&'a str, &'a [UnitLocation])>) -> Self {
        let rows: Vec<_> = rows.into_iter().collect();
        let mut files: Vec<_> = rows
            .iter()
            .flat_map(|(_, units)| units.iter())
            .map(|unit| (&*unit.path, &*unit.source_sha256))
            .collect();
        files.sort_unstable();
        files.dedup();
        Self { files, rows }
    }

    /// Refuse a map the reader would refuse: its exact encoded length over
    /// the file bound.
    pub(crate) fn within_bounds(&self) -> FResult<()> {
        within_publication_limit(serialized_bytes(self)?, MAX_META_BYTES, "mapping")
    }

    pub(crate) fn encode(&self) -> FResult<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    fn file(&self, unit: &UnitLocation) -> usize {
        self.files
            .binary_search(&(&*unit.path, &*unit.source_sha256))
            .expect("every location's file is in the table")
    }
}

#[cfg(feature = "semantic")]
impl Serialize for LabelTable<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        struct Units<'t, 'a>(&'t LabelTable<'a>, &'a [UnitLocation]);
        impl Serialize for Units<'_, '_> {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_seq(
                    self.1
                        .iter()
                        .map(|unit| (self.0.file(unit), unit.kind, unit.start, unit.end)),
                )
            }
        }
        struct Rows<'t, 'a>(&'t LabelTable<'a>);
        impl Serialize for Rows<'_, '_> {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_seq(
                    self.0
                        .rows
                        .iter()
                        .map(|(key, units)| (key, Units(self.0, units))),
                )
            }
        }
        let mut map = serializer.serialize_struct("LabelTable", 2)?;
        map.serialize_field("files", &self.files)?;
        map.serialize_field("labels", &Rows(self))?;
        map.end()
    }
}

type EncodedLocation = (usize, crate::neural::cache::CardKind, u64, u64);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedLabels {
    files: Vec<(Arc<str>, Arc<str>)>,
    labels: Vec<(String, Vec<EncodedLocation>)>,
}

/// Decode a v6 label map: files sorted and unique, every file index in range,
/// keys sorted and unique, and every key's locations valid for `recipe`.
/// Locations share their file's strings; cancellation is checked per 1,024
/// labels and locations together.
pub(super) fn decode_labels(
    bytes: &[u8],
    recipe: &str,
    control: &Control,
) -> Result<Locations, GenerationError> {
    let EncodedLabels { files, labels } =
        serde_json::from_slice(bytes).map_err(|e| unavailable(format!("label map: {e}")))?;
    let malformed = || unavailable("label map files, locations or ordinals are malformed");
    if !files.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(malformed());
    }
    let (mut units, mut previous) = (Locations::new(), None::<String>);
    let mut work = 0usize;
    let mut step = || -> Result<(), GenerationError> {
        work += 1;
        if work.is_multiple_of(1024) {
            control.check().map_err(GenerationError::Interrupted)?;
        }
        Ok(())
    };
    for (key, encoded) in labels {
        step()?;
        let mut locations = Vec::with_capacity(encoded.len());
        for (file, kind, start, end) in encoded {
            step()?;
            let (path, source_sha256) = files.get(file).ok_or_else(malformed)?;
            locations.push(UnitLocation {
                kind,
                path: path.clone(),
                start,
                end,
                source_sha256: source_sha256.clone(),
            });
        }
        if !crate::is_lower_hex(&key, 64)
            || previous.as_ref().is_some_and(|p| p >= &key)
            || !valid_units(&locations, recipe)
        {
            return Err(malformed());
        }
        previous = Some(key.clone());
        units.insert(key, Arc::new(locations));
    }
    Ok(units)
}
#[derive(Serialize, Deserialize)]
struct Change {
    key: String,
    units: Option<Arc<Vec<UnitLocation>>>,
}
#[cfg(feature = "semantic")]
pub(super) fn encode(overlay: &Overlay) -> FResult<Vec<u8>> {
    Ok(serde_json::to_vec(
        &overlay
            .iter()
            .map(|(key, units)| Change {
                key: key.clone(),
                units: units.clone(),
            })
            .collect::<Vec<_>>(),
    )?)
}
pub(super) fn valid_units(units: &[UnitLocation], recipe: &str) -> bool {
    !units.is_empty()
        && units.windows(2).all(|pair| {
            (&pair[0].path, pair[0].start, pair[0].end)
                < (&pair[1].path, pair[1].start, pair[1].end)
        })
        && units.iter().all(|u| {
            !u.path.is_empty()
                && u.start < u.end
                && crate::is_lower_hex(&u.source_sha256, 64)
                && u.kind.valid_for(recipe)
        })
}

pub(super) fn read(
    dir: &Dir,
    manifest: &mut GenerationManifest,
    cache: &mut verification::Cache,
    control: &Control,
) -> Result<(Locations, Locations, Overlay), GenerationError> {
    let open = |name: String| {
        dir.open_file(name)
            .map_err(|e| unavailable(format!("generation is partial or metadata unreadable: {e}")))
    };
    manifest.base_entries = verification::metadata(
        &mut cache.members,
        open(manifest.members_file())?,
        &manifest.members_sha256,
        MAX_META_BYTES,
        control,
        |bytes| {
            let keys: Vec<String> = serde_json::from_slice(bytes)
                .map_err(|e| unavailable(format!("base membership: {e}")))?;
            if !keys.iter().all(|k| crate::is_lower_hex(k, 64))
                || !keys.windows(2).all(|w| w[0] < w[1])
            {
                return Err(unavailable("base membership is not sorted and unique"));
            }
            Ok(Arc::new(keys))
        },
    )?;
    if manifest.base_entries.len() != manifest.base_count {
        return Err(unavailable("base membership count disagrees with manifest"));
    }
    let base = verification::metadata(
        &mut cache.locations,
        open(manifest.labels_file())?,
        &manifest.labels_sha256,
        MAX_META_BYTES,
        control,
        |bytes| decode_labels(bytes, &manifest.recipe_id, control),
    )?;
    let overlay = match &manifest.locations_delta {
        Some(hash) => verification::metadata(
            &mut cache.overlay,
            open(manifest.location_delta_file().expect("named overlay"))?,
            hash,
            MAX_DELTA_BYTES,
            control,
            |bytes| {
                let changes: Vec<Change> = serde_json::from_slice(bytes)
                    .map_err(|e| unavailable(format!("location overlay: {e}")))?;
                if changes.is_empty()
                    || changes.len() > MAX_CHANGES
                    || !changes.windows(2).all(|w| w[0].key < w[1].key)
                {
                    return Err(unavailable(
                        "location overlay is empty, unsorted or exceeds its bound",
                    ));
                }
                let mut overlay = Overlay::new();
                for change in changes {
                    if !crate::is_lower_hex(&change.key, 64)
                        || change
                            .units
                            .as_ref()
                            .is_some_and(|u| !valid_units(u, &manifest.recipe_id))
                    {
                        return Err(unavailable("location overlay is malformed"));
                    }
                    overlay.insert(change.key, change.units);
                }
                Ok(overlay)
            },
        )?,
        None => {
            cache.overlay = None;
            Overlay::new()
        }
    };
    // This full membership join runs only when either immutable base changes.
    let outside = match cache.outside.as_ref() {
        Some((members, labels, keys))
            if members == &manifest.members_sha256 && labels == &manifest.labels_sha256 =>
        {
            keys.clone()
        }
        _ => {
            let mut outside = BTreeSet::new();
            for (i, key) in base.keys().enumerate() {
                if i.is_multiple_of(128) {
                    control.check().map_err(GenerationError::Interrupted)?;
                }
                if manifest.base_entries.binary_search(key).is_err() {
                    outside.insert(key.clone());
                }
            }
            // A location base is compacted with at most one bounded ANN delta.
            if outside.len() > MAX_DELTA_ENTRIES {
                return Err(unavailable(
                    "location base exceeds non-base membership bound",
                ));
            }
            cache.outside = Some((
                manifest.members_sha256.clone(),
                manifest.labels_sha256.clone(),
                outside.clone(),
            ));
            outside
        }
    };
    let mut outside = outside;
    let id = manifest.mapping_id();
    let remembered = cache
        .snapshot
        .as_ref()
        .filter(|(key, _)| key == &id)
        .map(|(_, value)| value.clone());
    let mut active = remembered.clone().unwrap_or_else(|| base.clone());
    for (key, units) in &overlay {
        if base.get(key) == units.as_ref() {
            return Err(unavailable("location overlay contains a redundant change"));
        }
        if manifest.base_entries.binary_search(key).is_err() {
            if units.is_some() {
                outside.insert(key.clone());
            } else {
                outside.remove(key);
            }
        }
        if remembered.is_none() {
            match units {
                Some(units) => {
                    active.insert(key.clone(), units.clone());
                }
                None => {
                    active.remove(key);
                }
            }
        }
    }
    let delta = manifest
        .delta
        .as_ref()
        .map(|d| d.entries.as_slice())
        .unwrap_or_default();
    if active.len() != manifest.count
        || !outside.iter().eq(delta.iter())
        || active.len() < outside.len()
        || manifest.base_count < active.len() - outside.len()
        || manifest.base_count - (active.len() - outside.len()) > churn_limit(manifest.base_count)
    {
        return Err(unavailable(
            "active locations disagree with segment membership or counts",
        ));
    }
    cache.snapshot = Some((id, active.clone()));
    Ok((active, base, overlay))
}

#[cfg(all(test, feature = "semantic"))]
mod tests {
    use super::*;
    use crate::neural::cache::CardKind;

    const RECIPE: &str = "hierarchy-v1+overview4+context2+kinds2+units5:512+parent64+test";

    fn location(path: &str, hash: char, start: u64, kind: CardKind) -> UnitLocation {
        UnitLocation {
            kind,
            path: path.into(),
            start,
            end: start + 5,
            source_sha256: hash.to_string().repeat(64).into(),
        }
    }

    fn encode(rows: &Locations) -> Vec<u8> {
        LabelTable::new(
            rows.iter()
                .map(|(key, units)| (key.as_str(), units.as_slice())),
        )
        .encode()
        .unwrap()
    }

    fn decode(bytes: &[u8]) -> Result<Locations, GenerationError> {
        decode_labels(bytes, RECIPE, &Control::unbounded())
    }

    /// v6 stores each path and hash once. Serving must still see exactly the
    /// locations publication held, and a damaged table must be refused rather
    /// than resolve a location to another file.
    #[test]
    fn a_v6_label_map_decodes_the_locations_it_encoded_and_refuses_damage() {
        let odd = "a\"b\\c\nd\u{1}é🦀.rs";
        let mut rows = Locations::new();
        rows.insert(
            format!("{:064x}", 1),
            Arc::new(vec![
                location(odd, 'a', 0, CardKind::Body),
                location("src/lib.rs", 'b', 7, CardKind::Parent),
            ]),
        );
        // The same path at another source hash, and a file shared across keys.
        rows.insert(
            format!("{:064x}", 2),
            Arc::new(vec![
                location("src/lib.rs", 'c', 3, CardKind::File),
                location("src/lib.rs", 'c', 9, CardKind::ContainerFragment),
            ]),
        );
        rows.insert(
            format!("{:064x}", 3),
            Arc::new(vec![location(odd, 'a', 11, CardKind::Body)]),
        );
        let bytes = encode(&rows);
        assert_eq!(decode(&bytes).unwrap(), rows);
        assert_eq!(
            decode(&encode(&Locations::new())).unwrap(),
            Locations::new()
        );

        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            value["files"].as_array().unwrap().len(),
            3,
            "each file once"
        );
        let damaged = |edit: &dyn Fn(&mut serde_json::Value)| {
            let mut value = value.clone();
            edit(&mut value);
            decode(&serde_json::to_vec(&value).unwrap())
        };
        for (what, result) in [
            (
                "file index out of range",
                damaged(&|v| v["labels"][0][1][0][0] = 3.into()),
            ),
            (
                "unsorted files",
                damaged(&|v| v["files"].as_array_mut().unwrap().swap(0, 2)),
            ),
            (
                "unsorted keys",
                damaged(&|v| v["labels"].as_array_mut().unwrap().swap(0, 1)),
            ),
            (
                "a kind the recipe cannot carry",
                damaged(&|v| v["labels"][0][1][0][1] = "address".into()),
            ),
            (
                "a v5 map",
                damaged(&|v| *v = serde_json::json!({"labels": []})),
            ),
        ] {
            assert!(
                matches!(result, Err(GenerationError::Unavailable(_))),
                "{what}: {result:?}"
            );
        }
    }
}
