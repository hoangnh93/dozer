use dozer_cache::cache::{
    expression::{QueryExpression, Skip},
    RoCache,
};
use dozer_types::types::Record;

/// Validate if `query.skip` and `query.limit` works correctly by comparing the results
/// with the results of the same query with `skip` 0 and `limit` 1000.
///
/// Returns the query with `skip` 0 and `limit` 1000 and its results.
pub fn validate(
    cache: &dyn RoCache,
    schema_name: &str,
    mut query: QueryExpression,
) -> (QueryExpression, Vec<Record>) {
    let count = cache.count(schema_name, &query).unwrap();
    let records = cache.query(schema_name, &query).unwrap().1;

    let skip = query.skip;
    let limit = query.limit;

    query.skip = Skip::Skip(0);
    query.limit = None;
    let all_count = cache.count(schema_name, &query).unwrap();
    let all_records = cache.query(schema_name, &query).unwrap().1;

    let expected_count = match skip {
        Skip::Skip(skip) => (all_count - skip).min(limit.unwrap_or(usize::MAX)),
        Skip::After(id) => all_records
            .iter()
            .skip_while(|record| record.id != id)
            .skip(1)
            .take(limit.unwrap_or(usize::MAX))
            .count(),
    };
    let expected: Vec<_> = match skip {
        Skip::Skip(skip) => all_records
            .iter()
            .skip(skip)
            .take(limit.unwrap_or(usize::MAX))
            .cloned()
            .collect(),
        Skip::After(id) => all_records
            .iter()
            .skip_while(|record| record.id != id)
            .skip(1)
            .take(limit.unwrap_or(usize::MAX))
            .cloned()
            .collect(),
    };

    assert_eq!(count, expected_count);
    assert_eq!(records.len(), expected.len());
    for (record, expected) in records.into_iter().zip(expected) {
        assert_eq!(record, expected);
    }

    let all_records = all_records
        .into_iter()
        .map(|record| record.record)
        .collect();
    (query, all_records)
}
