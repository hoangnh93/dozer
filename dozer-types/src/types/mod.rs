use ahash::AHasher;
use geo::{point, GeodesicDistance, Point};
use ordered_float::OrderedFloat;
use std::array::TryFromSliceError;
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use crate::errors::types::TypeError;
use prettytable::{Cell, Row, Table};
use serde::{self, Deserialize, Serialize};

mod field;

use crate::errors::types::TypeError::InvalidFieldValue;
pub use field::{field_test_cases, Field, FieldBorrow, FieldType, DATE_FORMAT};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceDefinition {
    Table { connection: String, name: String },
    Alias { name: String },
    Dynamic,
}
impl Default for SourceDefinition {
    fn default() -> Self {
        SourceDefinition::Dynamic
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FieldDefinition {
    pub name: String,
    pub typ: FieldType,
    pub nullable: bool,
    #[serde(default)]
    pub source: SourceDefinition,
}

impl FieldDefinition {
    pub fn new(name: String, typ: FieldType, nullable: bool, source: SourceDefinition) -> Self {
        Self {
            name,
            typ,
            nullable,
            source,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct SchemaIdentifier {
    pub id: u32,
    pub version: u16,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Schema {
    /// Unique identifier and version for this schema. This value is required only if the schema
    /// is represented by a valid entry in the schema registry. For nested schemas, this field
    /// is not applicable
    pub identifier: Option<SchemaIdentifier>,

    /// fields contains a list of FieldDefinition for all the fields that appear in a record.
    /// Not necessarily all these fields will end up in the final object structure stored in
    /// the cache. Some fields might only be used for indexing purposes only.
    pub fields: Vec<FieldDefinition>,

    /// Indexes of the fields forming the primary key for this schema. If the value is empty
    /// only Insert Operation are supported. Updates and Deletes are not supported without a
    /// primary key definition
    #[serde(default)]
    pub primary_index: Vec<usize>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ReplicationChangesTrackingType {
    FullChanges,
    OnlyPK,
    Nothing,
}

impl Default for ReplicationChangesTrackingType {
    fn default() -> Self {
        ReplicationChangesTrackingType::Nothing
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SourceSchema {
    pub name: String,
    pub schema: Schema,
    #[serde(default)]
    pub replication_type: ReplicationChangesTrackingType,
}

impl SourceSchema {
    pub fn new(
        name: String,
        schema: Schema,
        replication_type: ReplicationChangesTrackingType,
    ) -> Self {
        Self {
            name,
            schema,
            replication_type,
        }
    }
}

impl Schema {
    pub fn empty() -> Schema {
        Self {
            identifier: None,
            fields: Vec::new(),
            primary_index: Vec::new(),
        }
    }

    pub fn field(&mut self, f: FieldDefinition, pk: bool) -> &mut Self {
        self.fields.push(f);
        if pk {
            self.primary_index.push(&self.fields.len() - 1)
        }
        self
    }

    pub fn get_field_index(&self, name: &str) -> Result<(usize, &FieldDefinition), TypeError> {
        let r = self
            .fields
            .iter()
            .enumerate()
            .find(|f| f.1.name.as_str() == name);
        match r {
            Some(v) => Ok(v),
            _ => Err(TypeError::InvalidFieldName(name.to_string())),
        }
    }

    pub fn print(&self) -> Table {
        let mut table = Table::new();
        table.add_row(row!["Field", "Type", "Nullable"]);
        for f in &self.fields {
            table.add_row(row![f.name, format!("{:?}", f.typ), f.nullable]);
        }
        table
    }

    pub fn set_identifier(
        &mut self,
        identifier: Option<SchemaIdentifier>,
    ) -> Result<(), TypeError> {
        self.identifier = identifier;
        Ok(())
    }
}

impl Display for Schema {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let table = self.print();
        table.fmt(f)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum IndexDefinition {
    /// The sorted inverted index, supporting `Eq` filter on multiple fields and `LT`, `LTE`, `GT`, `GTE` filter on at most one field.
    SortedInverted(Vec<usize>),
    /// Full text index, supporting `Contains`, `MatchesAny` and `MatchesAll` filter on exactly one field.
    FullText(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Record {
    /// Schema implemented by this Record
    pub schema_id: Option<SchemaIdentifier>,
    /// List of values, following the definitions of `fields` of the associated schema
    pub values: Vec<Field>,
    /// Records with same primary key will have increasing version.
    pub version: Option<u32>,
}

impl Record {
    pub fn new(
        schema_id: Option<SchemaIdentifier>,
        values: Vec<Field>,
        version: Option<u32>,
    ) -> Record {
        Record {
            schema_id,
            values,
            version,
        }
    }

    pub fn from_schema(schema: &Schema) -> Record {
        Record {
            schema_id: schema.identifier,
            values: vec![Field::Null; schema.fields.len()],
            version: None,
        }
    }

    pub fn nulls(schema_id: Option<SchemaIdentifier>, size: usize, version: Option<u32>) -> Record {
        Record {
            schema_id,
            values: vec![Field::Null; size],
            version,
        }
    }

    pub fn iter(&self) -> core::slice::Iter<'_, Field> {
        self.values.iter()
    }

    pub fn set_value(&mut self, idx: usize, value: Field) {
        self.values[idx] = value;
    }

    pub fn push_value(&mut self, value: Field) {
        self.values.push(value);
    }

    pub fn get_value(&self, idx: usize) -> Result<&Field, TypeError> {
        match self.values.get(idx) {
            Some(f) => Ok(f),
            _ => Err(TypeError::InvalidFieldIndex(idx)),
        }
    }

    pub fn get_key(&self, indexes: &Vec<usize>) -> Vec<u8> {
        debug_assert!(!indexes.is_empty(), "Primary key indexes cannot be empty");

        let mut tot_size = 0_usize;
        let mut buffers = Vec::<Vec<u8>>::with_capacity(indexes.len());
        for i in indexes {
            let bytes = self.values[*i].encode();
            tot_size += bytes.len();
            buffers.push(bytes);
        }

        let mut res_buffer = Vec::<u8>::with_capacity(tot_size);
        for i in buffers {
            res_buffer.extend(i);
        }
        res_buffer
    }

    pub fn get_values_hash(&self) -> u64 {
        let mut hasher = AHasher::default();

        for (index, field) in self.values.iter().enumerate() {
            hasher.write_i32(index as i32);
            match field {
                Field::UInt(i) => {
                    hasher.write_u8(1);
                    hasher.write_u64(*i);
                }
                Field::Int(i) => {
                    hasher.write_u8(2);
                    hasher.write_i64(*i);
                }
                Field::Float(f) => {
                    hasher.write_u8(3);
                    hasher.write(&((*f).to_ne_bytes()));
                }
                Field::Boolean(b) => {
                    hasher.write_u8(4);
                    hasher.write_u8(if *b { 1_u8 } else { 0_u8 });
                }
                Field::String(s) => {
                    hasher.write_u8(5);
                    hasher.write(s.as_str().as_bytes());
                }
                Field::Text(t) => {
                    hasher.write_u8(6);
                    hasher.write(t.as_str().as_bytes());
                }
                Field::Binary(b) => {
                    hasher.write_u8(7);
                    hasher.write(b.as_ref());
                }
                Field::Decimal(d) => {
                    hasher.write_u8(8);
                    hasher.write(&d.serialize());
                }
                Field::Timestamp(t) => {
                    hasher.write_u8(9);
                    hasher.write_i64(t.timestamp())
                }
                Field::Date(d) => {
                    hasher.write_u8(10);
                    hasher.write(d.to_string().as_bytes());
                }
                Field::Bson(b) => {
                    hasher.write_u8(11);
                    hasher.write(b.as_ref());
                }
                Field::Point(p) => {
                    hasher.write_u8(12);
                    hasher.write(p.to_bytes().as_slice());
                }
                Field::Null => {
                    hasher.write_u8(0);
                }
            }
        }
        hasher.finish()
    }
}

impl Display for Record {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let v = self
            .values
            .iter()
            .map(|f| Cell::new(&f.to_string().unwrap_or("".to_string())))
            .collect::<Vec<Cell>>();

        let mut table = Table::new();
        table.add_row(Row::new(v));
        table.fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    Delete { old: Record },
    Insert { new: Record },
    Update { old: Record, new: Record },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct DozerPoint(pub Point<OrderedFloat<f64>>);

impl GeodesicDistance<OrderedFloat<f64>> for DozerPoint {
    fn geodesic_distance(&self, rhs: &Self) -> OrderedFloat<f64> {
        let f = point! { x: self.0.x().0, y: self.0.y().0 };
        let t = point! { x: rhs.0.x().0, y: rhs.0.y().0 };
        OrderedFloat(f.geodesic_distance(&t))
    }
}

impl Ord for DozerPoint {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.0.x() == other.0.x() && self.0.y() == other.0.y() {
            Ordering::Equal
        } else if self.0.x() > other.0.x()
            || (self.0.x() == other.0.x() && self.0.y() > other.0.y())
        {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }
}

impl PartialOrd for DozerPoint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for DozerPoint {
    type Err = TypeError;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        let error = || InvalidFieldValue {
            field_type: FieldType::Point,
            nullable: false,
            value: str.to_string(),
        };

        let s = str.replace('(', "");
        let s = s.replace(')', "");
        let mut cs = s.split(',');
        let x = cs
            .next()
            .ok_or_else(error)?
            .parse::<f64>()
            .map_err(|_| error())?;
        let y = cs
            .next()
            .ok_or_else(error)?
            .parse::<f64>()
            .map_err(|_| error())?;
        Ok(Self(Point::from((OrderedFloat(x), OrderedFloat(y)))))
    }
}

impl From<(f64, f64)> for DozerPoint {
    fn from((x, y): (f64, f64)) -> Self {
        Self(point! {x: OrderedFloat(x), y: OrderedFloat(y)})
    }
}

impl Display for DozerPoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self.0.x_y()))
    }
}

impl DozerPoint {
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut result = [0_u8; 16];
        result[0..8].copy_from_slice(&self.0.x().to_be_bytes());
        result[8..16].copy_from_slice(&self.0.y().to_be_bytes());
        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TryFromSliceError> {
        let x = f64::from_be_bytes(bytes[0..8].try_into()?);
        let y = f64::from_be_bytes(bytes[8..16].try_into()?);

        Ok(DozerPoint::from((x, y)))
    }
}
