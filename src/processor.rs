use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use std::collections::BTreeMap;
use crate::scanner::ComObject;

/// Processes a vector of ComObjects by applying fuzzy matching based on the query
/// and grouping the results by the ProgID prefix (the part before the first dot).
///
/// # Arguments
/// * `objects` - A vector of ComObject instances to process.
/// * `query` - The search query string. If empty, all objects are included.
///
/// # Returns
/// A BTreeMap where keys are the ProgID prefixes and values are vectors of matching ComObjects,
/// sorted by fuzzy match score in descending order when a query is provided.
pub fn process_objects(objects: Vec<ComObject>, query: &str) -> BTreeMap<String, Vec<ComObject>> {
    let matcher = SkimMatcherV2::default();

    // Filter and score the objects based on fuzzy matching
    let mut scored: Vec<(i64, ComObject)> = objects
        .into_iter()
        .filter_map(|obj| {
            if query.is_empty() {
                return Some((0, obj));
            }

            let s_name = matcher.fuzzy_match(&obj.name, query).map(|s| s + 10);
            let s_clsid = matcher.fuzzy_match(&obj.clsid, query).map(|s| s + 5);
            let s_desc = matcher.fuzzy_match(&obj.description, query);

            let max_score = [s_name, s_clsid, s_desc]
                .iter()
                .filter_map(|&s| s)
                .max();
            max_score.map(|score| (score, obj))
        })
        .collect();

    // Sort by score descending if searching
    if !query.is_empty() {
        scored.sort_by(|a, b| b.0.cmp(&a.0));
    }

    // Group by ProgID prefix
    let mut groups: BTreeMap<String, Vec<ComObject>> = BTreeMap::new();
    for (_, obj) in scored {
        let prefix = obj.name.split('.').next().unwrap_or("Misc").to_string();
        groups.entry(prefix).or_default().push(obj);
    }

    // Sort within each group by name
    for group in groups.values_mut() {
        group.sort_by(|a, b| a.name.cmp(&b.name));
    }

    groups
}
