//! Immutable location base plus one bounded replacement/tombstone overlay.
//! Persistent imbl trees share unchanged nodes across publication and serving;
//! all durable bytes remain ordinary, content-bound generation artifacts.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub type Locations = imbl::OrdMap<String, Arc<Vec<UnitLocation>>>;
pub(crate) type Overlay = BTreeMap<String, Option<Arc<Vec<UnitLocation>>>>;
pub const MAX_CHANGES: usize = 4096;
pub const MAX_DELTA_BYTES: u64 = 32 * 1024 * 1024;
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
                && is_hex64(&u.source_sha256)
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
            if !keys.iter().all(|k| is_hex64(k)) || !keys.windows(2).all(|w| w[0] < w[1]) {
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
        |bytes| {
            let labels: LabelMap = serde_json::from_slice(bytes)
                .map_err(|e| unavailable(format!("label map: {e}")))?;
            let mut units = Locations::new();
            let mut previous: Option<String> = None;
            for (ordinal, entry) in labels.labels.into_iter().enumerate() {
                if ordinal.is_multiple_of(128) {
                    control.check().map_err(GenerationError::Interrupted)?;
                }
                if entry.label != ordinal as u64
                    || !is_hex64(&entry.input_key)
                    || previous.as_ref().is_some_and(|p| p >= &entry.input_key)
                    || !valid_units(&entry.units, &manifest.recipe_id)
                {
                    return Err(unavailable("label map locations or ordinals are malformed"));
                }
                previous = Some(entry.input_key.clone());
                units.insert(entry.input_key, Arc::new(entry.units));
            }
            Ok(units)
        },
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
                    if !is_hex64(&change.key)
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
