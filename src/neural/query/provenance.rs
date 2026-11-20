//! Bounded, fair provenance expansion of the ordinary dense hit window.
//! Locations still require the request's final source validation before delivery.
use super::*;

impl DenseWindow {
    /// The document-input key of one hit.
    pub fn input_key(&self, hit: &DenseHit) -> &str {
        self.generation.input_key(hit.label)
    }

    /// The unit locations one hit stood for at publication. Each is a
    /// candidate only after the request's final read revalidates it.
    pub fn units(&self, hit: &DenseHit) -> &[UnitLocation] {
        &self.generation.labels[self.input_key(hit)]
    }

    /// Admit at most `limit` ordinary provenance locations fairly across the
    /// bounded hit window. Filter lookup is logarithmic in each sorted map;
    /// duplicate provenance cannot force a corpus-wide expansion before caps.
    pub(crate) fn bounded_units(
        &self,
        filter: Option<&str>,
        limit: usize,
        control: &Control,
    ) -> Result<DenseLocations<'_>, Fallback> {
        let units = self
            .hits
            .iter()
            .take(DENSE_WINDOW)
            .map(|hit| self.units(hit))
            .collect();
        let mut locations = bounded_unit_locations(units, filter, limit, control)?;
        locations.truncated |= self.hits.len() > DENSE_WINDOW;
        Ok(locations)
    }

    /// The source revision a COMPLETE generation was published at; `None`
    /// for a generation that covers only part of the eligible units. The
    /// coverage word is `ready` only when this equals the request's revision.
    pub fn complete_at(&self) -> Option<u64> {
        self.generation
            .complete
            .then_some(self.generation.source_revision)
    }

    /// The coverage word of a response whose final read saw
    /// `source_revision`: `ready` only for a complete generation published
    /// at exactly that revision, otherwise `partial`. Known coverage, never
    /// inferred from the candidate set.
    pub fn coverage_at(&self, source_revision: u64) -> &'static str {
        if self.complete_at() == Some(source_revision) {
            "ready"
        } else {
            "partial"
        }
    }
}

pub(crate) struct DenseLocations<'a> {
    /// Fair admission order; the caller may restore hit order for placement.
    pub items: Vec<(usize, &'a UnitLocation)>,
    pub available: usize,
    pub truncated: bool,
}

fn bounded_unit_locations<'a>(
    units: Vec<&'a [UnitLocation]>,
    filter: Option<&str>,
    limit: usize,
    control: &Control,
) -> Result<DenseLocations<'a>, Fallback> {
    control.check()?;
    let subtree = filter.map(|path| (format!("{path}/"), format!("{path}0")));
    let mut available = 0usize;
    let mut cursors = Vec::with_capacity(units.len());
    for units in units {
        control.check()?;
        let (exact, children) = if let Some(path) = filter {
            // Exact path and component-boundary subtree are disjoint ranges:
            // e.g. a-b and a.rs can sort between a and a/child.
            let exact_from = units.partition_point(|unit| &*unit.path < path);
            let exact_to = units.partition_point(|unit| &*unit.path <= path);
            let (lower, upper) = subtree.as_ref().expect("filter subtree bounds");
            let child_from = units.partition_point(|unit| &*unit.path < lower.as_str());
            let child_to = units.partition_point(|unit| &*unit.path < upper.as_str());
            (&units[exact_from..exact_to], &units[child_from..child_to])
        } else {
            (units, &units[..0])
        };
        available = available
            .saturating_add(exact.len())
            .saturating_add(children.len());
        cursors.push(exact.iter().chain(children));
    }
    let mut items = Vec::with_capacity(limit.min(available));
    while items.len() < limit {
        let mut progressed = false;
        for (rank, cursor) in cursors.iter_mut().enumerate() {
            control.check()?;
            if let Some(location) = cursor.next() {
                items.push((rank, location));
                progressed = true;
                if items.len() == limit {
                    break;
                }
            }
        }
        if !progressed {
            break;
        }
    }
    Ok(DenseLocations {
        truncated: items.len() < available,
        items,
        available,
    })
}

#[cfg(all(test, feature = "test-faults"))]
mod tests {
    use super::*;

    #[test]
    fn ordinary_provenance_is_bounded_fair_filtered_and_cancellable() {
        let location = |path: &str| UnitLocation {
            kind: crate::neural::cache::CardKind::Body,
            path: path.into(),
            start: 0,
            end: 8,
            source_sha256: "ab".repeat(32).into(),
        };
        let shared: Vec<_> = (0..10_000)
            .map(|i| location(&format!("src/mirror_{i:05}.rs")))
            .collect();
        let independent = vec![
            location("src/independent_a.rs"),
            location("src/independent_b.rs"),
        ];
        let bounded = bounded_unit_locations(
            vec![&shared, &independent],
            None,
            4096,
            &Control::unbounded(),
        )
        .unwrap();
        assert_eq!(bounded.available, 10_002);
        assert_eq!(bounded.items.len(), 4096);
        assert!(bounded.truncated);
        assert_eq!(
            bounded.items.iter().filter(|(rank, _)| *rank == 1).count(),
            2
        );
        assert_eq!(&*bounded.items[1].1.path, "src/independent_a.rs");
        let filtered = bounded_unit_locations(
            vec![&shared],
            Some("src/mirror_09999.rs"),
            4096,
            &Control::unbounded(),
        )
        .unwrap();
        assert_eq!(filtered.items.len(), 1);
        assert_eq!(&*filtered.items[0].1.path, "src/mirror_09999.rs");
        assert!(!filtered.truncated);
        let paths: Vec<_> = ["a", "a-b", "a.rs", "a/x", "a/x/z", "ab", "b"]
            .into_iter()
            .map(location)
            .collect();
        let filtered =
            bounded_unit_locations(vec![&paths], Some("a"), 4096, &Control::unbounded()).unwrap();
        assert_eq!(
            filtered
                .items
                .iter()
                .map(|(_, unit)| &*unit.path)
                .collect::<Vec<_>>(),
            ["a", "a/x", "a/x/z"]
        );
        assert!(bounded_unit_locations(vec![&shared], None, 4096, &Control::cancelled()).is_err());
    }
}
