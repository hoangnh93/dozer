use crate::pipeline::aggregation::aggregator::AggregationResult;
use crate::pipeline::errors::PipelineError;
use crate::pipeline::errors::PipelineError::InvalidOperandType;
use crate::{deserialize_u8, to_bytes, try_unwrap};
use dozer_core::storage::common::Database;
use dozer_core::storage::prefix_transaction::PrefixTransaction;
use dozer_types::ordered_float::OrderedFloat;
use dozer_types::types::Field::{Date, Decimal, Float, Int, Timestamp, UInt};
use dozer_types::types::{Field, FieldType, DATE_FORMAT};

use crate::deserialize;
use dozer_types::chrono::{DateTime, FixedOffset, LocalResult, NaiveDate, TimeZone, Utc};
use dozer_types::errors::types::TypeError;
use std::string::ToString;

pub struct MaxAggregator {}
const AGGREGATOR_NAME: &str = "MAX";

impl MaxAggregator {
    const _AGGREGATOR_ID: u32 = 0x03;

    pub(crate) fn _get_type() -> u32 {
        MaxAggregator::_AGGREGATOR_ID
    }

    pub(crate) fn insert(
        _cur_state: Option<&[u8]>,
        new: &Field,
        return_type: FieldType,
        ptx: &mut PrefixTransaction,
        aggregators_db: Database,
    ) -> Result<AggregationResult, PipelineError> {
        match (return_type, new) {
            (FieldType::Date, _) => {
                // Update aggregators_db with new val and its occurrence
                let new_val = &Field::to_date(new)?.unwrap().to_string();
                Self::update_aggregator_db(new_val.as_bytes(), 1, false, ptx, aggregators_db);

                // Calculate minimum
                let maximum = try_unwrap!(Self::calc_date_max(ptx, aggregators_db));
                let min_date = NaiveDate::MIN;
                if maximum == min_date {
                    Ok(AggregationResult::new(Field::Null, None))
                } else {
                    Ok(AggregationResult::new(
                        Self::get_value(maximum.to_string().as_bytes(), return_type)?,
                        Some(Vec::from(maximum.to_string().as_bytes())),
                    ))
                }
            }
            (FieldType::Decimal, _) => {
                // Update aggregators_db with new val and its occurrence
                let new_val = &Field::to_decimal(new).unwrap().serialize();
                Self::update_aggregator_db(new_val.as_slice(), 1, false, ptx, aggregators_db);

                // Calculate minimum
                let maximum = try_unwrap!(Self::calc_decimal_max(ptx, aggregators_db));
                if maximum == dozer_types::rust_decimal::Decimal::MIN {
                    Ok(AggregationResult::new(Field::Null, None))
                } else {
                    Ok(AggregationResult::new(
                        Self::get_value(maximum.serialize().as_slice(), return_type)?,
                        Some(Vec::from(maximum.serialize())),
                    ))
                }
            }
            (FieldType::Float, _) => {
                // Update aggregators_db with new val and its occurrence
                let new_val = &OrderedFloat(Field::to_float(new).unwrap());
                Self::update_aggregator_db(to_bytes!(new_val), 1, false, ptx, aggregators_db);

                // Calculate average
                let maximum = try_unwrap!(Self::calc_f64_max(ptx, aggregators_db)).to_be_bytes();
                Ok(AggregationResult::new(
                    Self::get_value(&maximum, return_type)?,
                    Some(Vec::from(maximum)),
                ))
            }
            (FieldType::Int, _) => {
                // Update aggregators_db with new val and its occurrence
                let new_val = &Field::to_int(new).unwrap();
                Self::update_aggregator_db(to_bytes!(new_val), 1, false, ptx, aggregators_db);

                // Calculate average
                let maximum = try_unwrap!(Self::calc_i64_max(ptx, aggregators_db)).to_be_bytes();
                Ok(AggregationResult::new(
                    Self::get_value(&maximum, return_type)?,
                    Some(Vec::from(maximum)),
                ))
            }
            (FieldType::UInt, _) => {
                // Update aggregators_db with new val and its occurrence
                let new_val = &Field::to_uint(new).unwrap();
                Self::update_aggregator_db(to_bytes!(new_val), 1, false, ptx, aggregators_db);

                // Calculate average
                let maximum = try_unwrap!(Self::calc_u64_max(ptx, aggregators_db)).to_be_bytes();
                Ok(AggregationResult::new(
                    Self::get_value(&maximum, return_type)?,
                    Some(Vec::from(maximum)),
                ))
            }
            (FieldType::Timestamp, _) => {
                // Update aggregators_db with new val and its occurrence
                let new_val = &Field::to_timestamp(new)?
                    .unwrap()
                    .timestamp_millis()
                    .to_be_bytes();
                Self::update_aggregator_db(new_val.as_slice(), 1, false, ptx, aggregators_db);

                // Calculate minimum
                let maximum = try_unwrap!(Self::calc_timestamp_max(ptx, aggregators_db));
                let min_datetime: DateTime<FixedOffset> =
                    DateTime::from(DateTime::<FixedOffset>::MIN_UTC);
                if maximum == min_datetime {
                    Ok(AggregationResult::new(Field::Null, None))
                } else {
                    Ok(AggregationResult::new(
                        Self::get_value(
                            maximum.timestamp_millis().to_be_bytes().as_slice(),
                            return_type,
                        )?,
                        Some(Vec::from(maximum.timestamp_millis().to_be_bytes())),
                    ))
                }
            }
            _ => Err(InvalidOperandType(AGGREGATOR_NAME.to_string())),
        }
    }

    pub(crate) fn update(
        _cur_state: Option<&[u8]>,
        old: &Field,
        new: &Field,
        return_type: FieldType,
        ptx: &mut PrefixTransaction,
        aggregators_db: Database,
    ) -> Result<AggregationResult, PipelineError> {
        match (return_type, new) {
            (FieldType::Date, _) => {
                // Update aggregators_db with new val and its occurrence
                let new_val = &Field::to_date(new)?.unwrap().to_string();
                Self::update_aggregator_db(new_val.as_bytes(), 1, false, ptx, aggregators_db);
                let old_val = &Field::to_date(old)?.unwrap().to_string();
                Self::update_aggregator_db(old_val.as_bytes(), 1, true, ptx, aggregators_db);

                // Calculate minimum
                let maximum = try_unwrap!(Self::calc_date_max(ptx, aggregators_db));
                let min_date = NaiveDate::MIN;
                if maximum == min_date {
                    Ok(AggregationResult::new(Field::Null, None))
                } else {
                    Ok(AggregationResult::new(
                        Self::get_value(maximum.to_string().as_bytes(), return_type)?,
                        Some(Vec::from(maximum.to_string().as_bytes())),
                    ))
                }
            }
            (FieldType::Decimal, _) => {
                // Update aggregators_db with new val and its occurrence
                let new_val = &Field::to_decimal(new).unwrap().serialize();
                Self::update_aggregator_db(new_val.as_slice(), 1, false, ptx, aggregators_db);
                let old_val = &Field::to_decimal(old).unwrap().serialize();
                Self::update_aggregator_db(old_val.as_slice(), 1, true, ptx, aggregators_db);

                // Calculate minimum
                let maximum = try_unwrap!(Self::calc_decimal_max(ptx, aggregators_db));
                if maximum == dozer_types::rust_decimal::Decimal::MIN {
                    Ok(AggregationResult::new(Field::Null, None))
                } else {
                    Ok(AggregationResult::new(
                        Self::get_value(maximum.serialize().as_slice(), return_type)?,
                        Some(Vec::from(maximum.serialize())),
                    ))
                }
            }
            (FieldType::Float, _) => {
                // Update aggregators_db with new val and its occurrence
                let new_val = &OrderedFloat(Field::to_float(new).unwrap());
                Self::update_aggregator_db(to_bytes!(new_val), 1, false, ptx, aggregators_db);
                let old_val = &OrderedFloat(Field::to_float(old).unwrap());
                Self::update_aggregator_db(to_bytes!(old_val), 1, true, ptx, aggregators_db);

                // Calculate average
                let maximum = try_unwrap!(Self::calc_f64_max(ptx, aggregators_db)).to_be_bytes();
                Ok(AggregationResult::new(
                    Self::get_value(&maximum, return_type)?,
                    Some(Vec::from(maximum)),
                ))
            }
            (FieldType::Int, _) => {
                // Update aggregators_db with new val and its occurrence
                let new_val = &Field::to_int(new).unwrap();
                Self::update_aggregator_db(to_bytes!(new_val), 1, false, ptx, aggregators_db);
                let old_val = &Field::to_int(old).unwrap();
                Self::update_aggregator_db(to_bytes!(old_val), 1, true, ptx, aggregators_db);

                // Calculate average
                let maximum = (try_unwrap!(Self::calc_i64_max(ptx, aggregators_db))).to_be_bytes();
                Ok(AggregationResult::new(
                    Self::get_value(&maximum, return_type)?,
                    Some(Vec::from(maximum)),
                ))
            }
            (FieldType::UInt, _) => {
                // Update aggregators_db with new val and its occurrence
                let new_val = &Field::to_uint(new).unwrap();
                Self::update_aggregator_db(to_bytes!(new_val), 1, false, ptx, aggregators_db);
                let old_val = &Field::to_uint(old).unwrap();
                Self::update_aggregator_db(to_bytes!(old_val), 1, true, ptx, aggregators_db);

                // Calculate average
                let maximum = (try_unwrap!(Self::calc_u64_max(ptx, aggregators_db))).to_be_bytes();
                Ok(AggregationResult::new(
                    Self::get_value(&maximum, return_type)?,
                    Some(Vec::from(maximum)),
                ))
            }
            (FieldType::Timestamp, _) => {
                // Update aggregators_db with new val and its occurrence
                let new_val = &Field::to_timestamp(new)?
                    .unwrap()
                    .timestamp_millis()
                    .to_be_bytes();
                Self::update_aggregator_db(new_val.as_slice(), 1, false, ptx, aggregators_db);
                let old_val = &Field::to_timestamp(old)?
                    .unwrap()
                    .timestamp_millis()
                    .to_be_bytes();
                Self::update_aggregator_db(old_val.as_slice(), 1, true, ptx, aggregators_db);

                // Calculate maximum
                let maximum = try_unwrap!(Self::calc_timestamp_max(ptx, aggregators_db));
                let min_datetime: DateTime<FixedOffset> =
                    DateTime::from(DateTime::<FixedOffset>::MIN_UTC);
                if maximum == min_datetime {
                    Ok(AggregationResult::new(Field::Null, None))
                } else {
                    Ok(AggregationResult::new(
                        Self::get_value(
                            maximum.timestamp_millis().to_be_bytes().as_slice(),
                            return_type,
                        )?,
                        Some(Vec::from(maximum.timestamp_millis().to_be_bytes())),
                    ))
                }
            }
            _ => Err(InvalidOperandType(AGGREGATOR_NAME.to_string())),
        }
    }

    pub(crate) fn delete(
        _cur_state: Option<&[u8]>,
        old: &Field,
        return_type: FieldType,
        ptx: &mut PrefixTransaction,
        aggregators_db: Database,
    ) -> Result<AggregationResult, PipelineError> {
        match (return_type, old) {
            (FieldType::Date, _) => {
                // Update aggregators_db with new val and its occurrence
                let old_val = &Field::to_date(old)?.unwrap().to_string();
                Self::update_aggregator_db(old_val.as_bytes(), 1, true, ptx, aggregators_db);

                // Calculate minimum
                let maximum = try_unwrap!(Self::calc_date_max(ptx, aggregators_db));
                let min_date = NaiveDate::MIN;
                if maximum == min_date {
                    Ok(AggregationResult::new(Field::Null, None))
                } else {
                    Ok(AggregationResult::new(
                        Self::get_value(maximum.to_string().as_bytes(), return_type)?,
                        Some(Vec::from(maximum.to_string().as_bytes())),
                    ))
                }
            }
            (FieldType::Decimal, _) => {
                // Update aggregators_db with new val and its occurrence
                let old_val = &Field::to_decimal(old).unwrap().serialize();
                Self::update_aggregator_db(old_val.as_slice(), 1, true, ptx, aggregators_db);

                // Calculate minimum
                let maximum = try_unwrap!(Self::calc_decimal_max(ptx, aggregators_db));
                if maximum == dozer_types::rust_decimal::Decimal::MIN {
                    Ok(AggregationResult::new(Field::Null, None))
                } else {
                    Ok(AggregationResult::new(
                        Self::get_value(maximum.serialize().as_slice(), return_type)?,
                        Some(Vec::from(maximum.serialize())),
                    ))
                }
            }
            (FieldType::Float, _) => {
                // Update aggregators_db with new val and its occurrence
                let old_val = &OrderedFloat(Field::to_float(old).unwrap());
                Self::update_aggregator_db(to_bytes!(old_val), 1, true, ptx, aggregators_db);

                // Calculate average
                let maximum = try_unwrap!(Self::calc_f64_max(ptx, aggregators_db));
                if maximum == f64::MIN {
                    Ok(AggregationResult::new(Field::Null, None))
                } else {
                    Ok(AggregationResult::new(
                        Self::get_value(&maximum.to_be_bytes(), return_type)?,
                        Some(Vec::from(maximum.to_be_bytes())),
                    ))
                }
            }
            (FieldType::Int, _) => {
                // Update aggregators_db with new val and its occurrence
                let old_val = &Field::to_int(old).unwrap();
                Self::update_aggregator_db(to_bytes!(old_val), 1, true, ptx, aggregators_db);

                // Calculate average
                let maximum = try_unwrap!(Self::calc_i64_max(ptx, aggregators_db));
                if maximum == i64::MIN {
                    Ok(AggregationResult::new(Field::Null, None))
                } else {
                    Ok(AggregationResult::new(
                        Self::get_value(&maximum.to_be_bytes(), return_type)?,
                        Some(Vec::from(maximum.to_be_bytes())),
                    ))
                }
            }
            (FieldType::UInt, _) => {
                // Update aggregators_db with new val and its occurrence
                let old_val = &Field::to_uint(old).unwrap();
                Self::update_aggregator_db(to_bytes!(old_val), 1, true, ptx, aggregators_db);

                // Calculate average
                let maximum = try_unwrap!(Self::calc_u64_max(ptx, aggregators_db));
                if maximum == u64::MIN {
                    Ok(AggregationResult::new(Field::Null, None))
                } else {
                    Ok(AggregationResult::new(
                        Self::get_value(&maximum.to_be_bytes(), return_type)?,
                        Some(Vec::from(maximum.to_be_bytes())),
                    ))
                }
            }
            (FieldType::Timestamp, _) => {
                // Update aggregators_db with new val and its occurrence
                let old_val = &Field::to_timestamp(old)?
                    .unwrap()
                    .timestamp_millis()
                    .to_be_bytes();
                Self::update_aggregator_db(old_val.as_slice(), 1, true, ptx, aggregators_db);

                // Calculate maximum
                let maximum = try_unwrap!(Self::calc_timestamp_max(ptx, aggregators_db));
                let min_datetime: DateTime<FixedOffset> =
                    DateTime::from(DateTime::<FixedOffset>::MIN_UTC);
                if maximum == min_datetime {
                    Ok(AggregationResult::new(Field::Null, None))
                } else {
                    Ok(AggregationResult::new(
                        Self::get_value(
                            maximum.timestamp_millis().to_be_bytes().as_slice(),
                            return_type,
                        )?,
                        Some(Vec::from(maximum.timestamp_millis().to_be_bytes())),
                    ))
                }
            }
            _ => Err(InvalidOperandType(AGGREGATOR_NAME.to_string())),
        }
    }

    pub(crate) fn get_value(f: &[u8], from: FieldType) -> Result<Field, PipelineError> {
        match from {
            FieldType::Date => Ok(Date(
                NaiveDate::parse_from_str(
                    String::from_utf8(deserialize!(f)).unwrap().as_ref(),
                    DATE_FORMAT,
                )
                .unwrap(),
            )),
            FieldType::Decimal => Ok(Decimal(dozer_types::rust_decimal::Decimal::deserialize(
                deserialize!(f),
            ))),
            FieldType::Float => Ok(Float(OrderedFloat(f64::from_be_bytes(deserialize!(f))))),
            FieldType::Int => Ok(Int(i64::from_be_bytes(deserialize!(f)))),
            FieldType::UInt => Ok(UInt(u64::from_be_bytes(deserialize!(f)))),
            FieldType::Timestamp => {
                match Utc.timestamp_millis_opt(i64::from_be_bytes(deserialize!(f))) {
                    LocalResult::None => Err(PipelineError::InternalTypeError(
                        TypeError::InvalidTimestamp,
                    )),
                    LocalResult::Single(v) => Ok(Timestamp(DateTime::from(v))),
                    LocalResult::Ambiguous(_, _) => Err(PipelineError::InternalTypeError(
                        TypeError::AmbiguousTimestamp,
                    )),
                }
            }
            _ => Ok(Field::Null),
        }
    }

    fn update_aggregator_db(
        key: &[u8],
        val_delta: u8,
        decr: bool,
        ptx: &mut PrefixTransaction,
        aggregators_db: Database,
    ) {
        let get_prev_count = try_unwrap!(ptx.get(aggregators_db, key));
        let prev_count = deserialize_u8!(get_prev_count);
        let mut new_count = prev_count;
        if decr {
            new_count = new_count.wrapping_sub(val_delta);
        } else {
            new_count = new_count.wrapping_add(val_delta);
        }
        if new_count < 1 {
            try_unwrap!(ptx.del(aggregators_db, key, Option::from(to_bytes!(prev_count))));
        } else {
            try_unwrap!(ptx.put(aggregators_db, key, to_bytes!(new_count)));
        }
    }

    fn calc_f64_max(
        ptx: &mut PrefixTransaction,
        aggregators_db: Database,
    ) -> Result<f64, PipelineError> {
        let ptx_cur = ptx.open_cursor(aggregators_db)?;
        let mut maximum = f64::MIN;

        // get first to get the maximum
        if ptx_cur.last()? {
            let cur = try_unwrap!(ptx_cur.read()).unwrap();
            maximum = f64::from_be_bytes(deserialize!(cur.0));
        }
        Ok(maximum)
    }

    fn calc_decimal_max(
        ptx: &mut PrefixTransaction,
        aggregators_db: Database,
    ) -> Result<dozer_types::rust_decimal::Decimal, PipelineError> {
        let ptx_cur = ptx.open_cursor(aggregators_db)?;
        let mut maximum = dozer_types::rust_decimal::Decimal::MIN;

        // get first to get the minimum
        if ptx_cur.last()? {
            let cur = try_unwrap!(ptx_cur.read()).unwrap();
            maximum = dozer_types::rust_decimal::Decimal::deserialize(deserialize!(cur.0));
        }
        Ok(maximum)
    }

    fn calc_timestamp_max(
        ptx: &mut PrefixTransaction,
        aggregators_db: Database,
    ) -> Result<DateTime<FixedOffset>, PipelineError> {
        let ptx_cur = ptx.open_cursor(aggregators_db)?;
        let mut maximum = DateTime::<FixedOffset>::MIN_UTC;

        // get first to get the minimum
        if ptx_cur.last()? {
            let cur = try_unwrap!(ptx_cur.read()).unwrap();
            maximum = match Utc.timestamp_millis_opt(i64::from_be_bytes(deserialize!(cur.0))) {
                LocalResult::Single(v) => Ok(v),
                LocalResult::Ambiguous(_, _) => Err(PipelineError::InternalTypeError(
                    TypeError::AmbiguousTimestamp,
                )),
                LocalResult::None => Err(PipelineError::InternalTypeError(
                    TypeError::InvalidTimestamp,
                )),
            }?;
        }
        Ok(DateTime::from(maximum))
    }

    fn calc_date_max(
        ptx: &mut PrefixTransaction,
        aggregators_db: Database,
    ) -> Result<NaiveDate, PipelineError> {
        let ptx_cur = ptx.open_cursor(aggregators_db)?;
        let mut maximum = NaiveDate::MIN;

        // get first to get the minimum
        if ptx_cur.last()? {
            let cur = try_unwrap!(ptx_cur.read()).unwrap();
            maximum = NaiveDate::parse_from_str(
                String::from_utf8(deserialize!(cur.0)).unwrap().as_ref(),
                DATE_FORMAT,
            )
            .unwrap();
        }
        Ok(maximum)
    }

    fn calc_i64_max(
        ptx: &mut PrefixTransaction,
        aggregators_db: Database,
    ) -> Result<i64, PipelineError> {
        let ptx_cur = ptx.open_cursor(aggregators_db)?;
        let mut maximum = i64::MIN;

        // get first to get the maximum
        if ptx_cur.last()? {
            let cur = try_unwrap!(ptx_cur.read()).unwrap();
            maximum = i64::from_be_bytes(deserialize!(cur.0));
        }
        Ok(maximum)
    }

    fn calc_u64_max(
        ptx: &mut PrefixTransaction,
        aggregators_db: Database,
    ) -> Result<u64, PipelineError> {
        let ptx_cur = ptx.open_cursor(aggregators_db)?;
        let mut maximum = u64::MIN;

        // get first to get the maximum
        if ptx_cur.last()? {
            let cur = try_unwrap!(ptx_cur.read()).unwrap();
            maximum = u64::from_be_bytes(deserialize!(cur.0));
        }
        Ok(maximum)
    }
}
