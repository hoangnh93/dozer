use std::collections::HashMap;

use dozer_types::{
    grpc_types::ingest::{IngestArrowRequest, IngestRequest},
    types::{Schema, SchemaIdentifier, SourceSchema},
};

use crate::{errors::ConnectorError, ingestion::Ingestor};

mod default;

mod arrow;

pub use arrow::ArrowAdapter;
pub use default::DefaultAdapter;
pub trait IngestAdapter
where
    Self: Send + Sync + 'static + Sized,
{
    fn new() -> Self;
    fn get_schemas(&self, schemas_str: &str) -> Result<Vec<SourceSchema>, ConnectorError>;
    fn handle_message(
        &self,
        msg: GrpcIngestMessage,
        schema_map: &'static HashMap<String, Schema>,
        ingestor: &'static Ingestor,
    ) -> Result<(), ConnectorError>;
}

pub enum GrpcIngestMessage {
    Default(IngestRequest),
    Arrow(IngestArrowRequest),
}
pub struct GrpcIngestor<A>
where
    A: IngestAdapter,
{
    adapter: A,
    schemas_str: String,
    pub schema_map: &'static HashMap<String, Schema>,
}
impl<T> GrpcIngestor<T>
where
    T: IngestAdapter,
{
    pub fn new(schemas_str: String) -> Result<Self, ConnectorError> {
        let adapter = T::new();
        let schemas = adapter.get_schemas(&schemas_str)?;
        let schema_map = Self::get_schema_map(schemas)?;

        Ok(Self {
            schemas_str,
            schema_map: Box::leak(Box::new(schema_map)),
            adapter,
        })
    }
    pub fn get_schema_map(
        schemas: Vec<SourceSchema>,
    ) -> Result<HashMap<String, Schema>, ConnectorError> {
        Ok(schemas
            .into_iter()
            .enumerate()
            .map(|(id, mut v)| {
                v.schema.identifier = Some(SchemaIdentifier {
                    id: id as u32,
                    version: 1,
                });
                (v.name, v.schema)
            })
            .collect())
    }
}

impl<A> GrpcIngestor<A>
where
    A: IngestAdapter,
{
    pub fn get_schemas(&self) -> Result<Vec<SourceSchema>, ConnectorError> {
        self.adapter.get_schemas(&self.schemas_str)
    }

    pub fn handle_message(
        &self,
        msg: GrpcIngestMessage,
        ingestor: &'static Ingestor,
    ) -> Result<(), ConnectorError> {
        self.adapter.handle_message(msg, self.schema_map, ingestor)
    }
}
