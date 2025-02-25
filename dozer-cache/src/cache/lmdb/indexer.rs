use crate::errors::{CacheError, IndexError};
use dozer_storage::lmdb::RwTransaction;
use dozer_types::types::{Field, IndexDefinition, Record, Schema};
use itertools::Itertools;
use unicode_segmentation::UnicodeSegmentation;

use crate::cache::index::{self, get_full_text_secondary_index};

use super::cache::SecondaryIndexDatabases;

pub struct Indexer<'a> {
    pub secondary_indexes: &'a SecondaryIndexDatabases,
}
impl<'a> Indexer<'a> {
    pub fn build_indexes(
        &self,
        txn: &mut RwTransaction,
        record: &Record,
        schema: &Schema,
        secondary_indexes: &[IndexDefinition],
        id: [u8; 8],
    ) -> Result<(), CacheError> {
        let schema_id = schema.identifier.ok_or(CacheError::SchemaHasNoIdentifier)?;

        if secondary_indexes.is_empty() {
            return Err(CacheError::Index(IndexError::MissingSecondaryIndexes));
        }
        for (idx, index) in secondary_indexes.iter().enumerate() {
            let db = *self
                .secondary_indexes
                .get(&(schema_id, idx))
                .ok_or(CacheError::SecondaryIndexDatabaseNotFound)?;

            match index {
                IndexDefinition::SortedInverted(fields) => {
                    let secondary_key = Self::_build_index_sorted_inverted(fields, &record.values);
                    db.insert(txn, &secondary_key, id)?;
                }
                IndexDefinition::FullText(field_index) => {
                    for secondary_key in
                        Self::_build_indices_full_text(*field_index, &record.values)?
                    {
                        db.insert(txn, &secondary_key, id)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn delete_indexes(
        &self,
        txn: &mut RwTransaction,
        record: &Record,
        schema: &Schema,
        secondary_indexes: &[IndexDefinition],
        id: [u8; 8],
    ) -> Result<(), CacheError> {
        let schema_id = schema.identifier.ok_or(CacheError::SchemaHasNoIdentifier)?;
        for (idx, index) in secondary_indexes.iter().enumerate() {
            let db = *self
                .secondary_indexes
                .get(&(schema_id, idx))
                .ok_or(CacheError::SecondaryIndexDatabaseNotFound)?;

            match index {
                IndexDefinition::SortedInverted(fields) => {
                    let secondary_key = Self::_build_index_sorted_inverted(fields, &record.values);
                    db.delete(txn, &secondary_key, id)?;
                }
                IndexDefinition::FullText(field_index) => {
                    for secondary_key in
                        Self::_build_indices_full_text(*field_index, &record.values)?
                    {
                        db.delete(txn, &secondary_key, id)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn _build_index_sorted_inverted(fields: &[usize], values: &[Field]) -> Vec<u8> {
        let values = fields
            .iter()
            .copied()
            .filter_map(|index| (values.get(index)))
            .collect::<Vec<_>>();
        // `values.len() == 1` criteria must be kept the same with `comparator.rs`.
        index::get_secondary_index(&values, values.len() == 1)
    }

    fn _build_indices_full_text(
        field_index: usize,
        values: &[Field],
    ) -> Result<Vec<Vec<u8>>, CacheError> {
        let Some(field) = values.get(field_index) else {
            return Err(CacheError::Index(IndexError::FieldIndexOutOfRange));
        };

        let string = match field {
            Field::String(string) => string,
            Field::Text(string) => string,
            Field::Null => "",
            _ => {
                return Err(CacheError::Index(IndexError::FieldNotCompatibleIndex(
                    field_index,
                )))
            }
        };

        Ok(string
            .unicode_words()
            .map(get_full_text_secondary_index)
            .unique()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::cache::{
        lmdb::{
            cache::LmdbRwCache,
            tests::utils::{self as lmdb_utils, create_cache},
        },
        test_utils, RwCache,
    };

    use super::*;

    #[test]
    fn test_secondary_indexes() {
        let schema_name = "sample";
        let (cache, schema, secondary_indexes) = create_cache(schema_name, test_utils::schema_1);

        let items = vec![
            (1, Some("a".to_string()), Some(521)),
            (2, Some("a".to_string()), None),
            (3, None, Some(521)),
            (4, None, None),
        ];

        for val in items.clone() {
            lmdb_utils::insert_rec_1(&cache, &schema, val);
        }
        // No of index dbs
        let indexes = lmdb_utils::get_indexes(&cache);

        let index_count = indexes.iter().flatten().count();
        let expected_count = secondary_indexes.len();
        // 3 columns, 1 compound, 1 descending
        assert_eq!(
            indexes.len(),
            expected_count,
            "Must create db for each index"
        );

        assert_eq!(
            index_count,
            items.len() * expected_count,
            "Must index each field"
        );

        for a in [1i64, 2, 3, 4] {
            cache.delete(&Field::Int(a).encode()).unwrap();
        }

        assert_eq!(
            lmdb_utils::get_indexes(&cache)
                .into_iter()
                .flatten()
                .count(),
            0,
            "Must delete every index"
        );
    }

    #[test]
    fn test_build_indices_full_text() {
        let field_index = 0;
        assert_eq!(
            Indexer::_build_indices_full_text(
                field_index,
                &[Field::String("today is a good day".into())]
            )
            .unwrap(),
            vec![
                get_full_text_secondary_index("today"),
                get_full_text_secondary_index("is"),
                get_full_text_secondary_index("a"),
                get_full_text_secondary_index("good"),
                get_full_text_secondary_index("day"),
            ]
        );
    }

    #[test]
    fn test_full_text_secondary_index_with_duplicated_words() {
        let schema_name = "sample";
        let (schema, secondary_indexes) = test_utils::schema_full_text();
        let cache = LmdbRwCache::create(
            [(schema_name.to_string(), schema.clone(), secondary_indexes)],
            Default::default(),
            Default::default(),
        )
        .unwrap();

        let items = vec![(
            Some("another test".to_string()),
            Some("regular test regular".to_string()),
        )];

        for val in items {
            lmdb_utils::insert_full_text(&cache, &schema, val);
        }

        {
            let a = "another test".to_string();
            cache.delete(&Field::String(a).encode()).unwrap();
        }

        assert_eq!(
            lmdb_utils::get_indexes(&cache)
                .into_iter()
                .flatten()
                .count(),
            0,
            "Must delete every index"
        );
    }
}
