//! Documentation coverage checks.

use mdk_lint::RULES;

#[test]
fn every_rule_is_documented() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/rules.md");
    let docs = std::fs::read_to_string(path).expect("read docs/rules.md");
    let mut missing = Vec::new();
    for rule in RULES {
        let meta = rule.meta();
        if !docs.contains(meta.id) {
            missing.push(meta.id);
        }
    }
    assert!(
        missing.is_empty(),
        "rules missing from docs/rules.md: {missing:?}"
    );
}

#[test]
fn rule_registry_is_sorted_and_unique() {
    let ids: Vec<&str> = RULES.iter().map(|rule| rule.meta().id).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "rules must be registered in id order");
    let mut deduped = ids.clone();
    deduped.dedup();
    assert_eq!(ids.len(), deduped.len(), "duplicate rule ids");
}
