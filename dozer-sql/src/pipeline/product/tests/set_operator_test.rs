use crate::pipeline::builder::{statement_to_pipeline, SchemaSQLContext};
use dozer_core::app::{App, AppPipeline};
use dozer_core::appsource::{AppSource, AppSourceManager};
use dozer_core::channels::SourceChannelForwarder;
use dozer_core::epoch::Epoch;
use dozer_core::errors::ExecutionError;
use dozer_core::executor::{DagExecutor, ExecutorOptions};
use dozer_core::node::{
    OutputPortDef, OutputPortType, PortHandle, Sink, SinkFactory, Source, SourceFactory,
};
use dozer_core::record_store::RecordReader;
use dozer_core::storage::lmdb_storage::SharedTransaction;
use dozer_core::DEFAULT_PORT_HANDLE;
use dozer_types::chrono::NaiveDate;
use dozer_types::ingestion_types::IngestionMessage;
use dozer_types::log::debug;
use dozer_types::node::SourceStates;
use dozer_types::types::{
    Field, FieldDefinition, FieldType, Operation, Record, Schema, SourceDefinition,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tempdir::TempDir;

#[test]
fn test_set_union_pipeline_builder() {
    let sql = "WITH supplier_id_union AS (
                        SELECT supplier_id
                        FROM suppliers
                        UNION
                        SELECT supplier_id
                        FROM orders
                    )
                    SELECT supplier_id
                    INTO set_results
                    FROM supplier_id_union;";

    let mut pipeline: AppPipeline<SchemaSQLContext> = AppPipeline::new();
    let query_ctx =
        statement_to_pipeline(sql, &mut pipeline, Some("set_results".to_string())).unwrap();

    let table_info = query_ctx.output_tables_map.get("set_results").unwrap();
    let latch = Arc::new(AtomicBool::new(true));

    let mut asm = AppSourceManager::new();
    asm.add(AppSource::new(
        "connection".to_string(),
        Arc::new(TestSourceFactory::new(latch.clone())),
        vec![
            ("suppliers".to_string(), SUPPLIERS_PORT),
            ("orders".to_string(), ORDERS_PORT),
        ]
        .into_iter()
        .collect(),
    ))
    .unwrap();

    pipeline.add_sink(Arc::new(TestSinkFactory::new(7, latch)), "sink");
    pipeline
        .connect_nodes(
            &table_info.node,
            Some(table_info.port),
            "sink",
            Some(DEFAULT_PORT_HANDLE),
            true,
        )
        .unwrap();

    let mut app = App::new(asm);
    app.add_pipeline(pipeline);

    let dag = app.get_dag().unwrap();

    let tmp_dir = TempDir::new("example").unwrap_or_else(|_e| panic!("Unable to create temp dir"));
    if tmp_dir.path().exists() {
        std::fs::remove_dir_all(tmp_dir.path())
            .unwrap_or_else(|_e| panic!("Unable to remove old dir"));
    }
    std::fs::create_dir(tmp_dir.path()).unwrap_or_else(|_e| panic!("Unable to create temp dir"));

    use std::time::Instant;
    let now = Instant::now();

    let tmp_dir = TempDir::new("test").unwrap();

    DagExecutor::new(
        dag,
        tmp_dir.path().to_path_buf(),
        ExecutorOptions::default(),
    )
    .unwrap()
    .start(Arc::new(AtomicBool::new(true)))
    .unwrap()
    .join()
    .unwrap();

    let elapsed = now.elapsed();
    debug!("Elapsed: {:.2?}", elapsed);
}

#[test]
fn test_set_union_all_pipeline_builder() {
    let sql = "WITH supplier_id_union AS (
                        SELECT supplier_id
                        FROM suppliers
                        UNION ALL
                        SELECT supplier_id
                        FROM orders
                    )
                    SELECT supplier_id
                    INTO set_results
                    FROM supplier_id_union;";

    dozer_tracing::init_telemetry(false).unwrap();

    let mut pipeline: AppPipeline<SchemaSQLContext> = AppPipeline::new();
    let query_ctx =
        statement_to_pipeline(sql, &mut pipeline, Some("set_results".to_string())).unwrap();

    let table_info = query_ctx.output_tables_map.get("set_results").unwrap();
    let latch = Arc::new(AtomicBool::new(true));

    let mut asm = AppSourceManager::new();
    asm.add(AppSource::new(
        "connection".to_string(),
        Arc::new(TestSourceFactory::new(latch.clone())),
        vec![
            ("suppliers".to_string(), SUPPLIERS_PORT),
            ("orders".to_string(), ORDERS_PORT),
        ]
        .into_iter()
        .collect(),
    ))
    .unwrap();

    pipeline.add_sink(Arc::new(TestSinkFactory::new(7, latch)), "sink");
    pipeline
        .connect_nodes(
            &table_info.node,
            Some(table_info.port),
            "sink",
            Some(DEFAULT_PORT_HANDLE),
            true,
        )
        .unwrap();

    let mut app = App::new(asm);
    app.add_pipeline(pipeline);

    let dag = app.get_dag().unwrap();

    let tmp_dir = TempDir::new("example").unwrap_or_else(|_e| panic!("Unable to create temp dir"));
    if tmp_dir.path().exists() {
        std::fs::remove_dir_all(tmp_dir.path())
            .unwrap_or_else(|_e| panic!("Unable to remove old dir"));
    }
    std::fs::create_dir(tmp_dir.path()).unwrap_or_else(|_e| panic!("Unable to create temp dir"));

    use std::time::Instant;
    let now = Instant::now();

    let tmp_dir = TempDir::new("test").unwrap();

    DagExecutor::new(
        dag,
        tmp_dir.path().to_path_buf(),
        ExecutorOptions::default(),
    )
    .unwrap()
    .start(Arc::new(AtomicBool::new(true)))
    .unwrap()
    .join()
    .unwrap();

    let elapsed = now.elapsed();
    debug!("Elapsed: {:.2?}", elapsed);
}

const SUPPLIERS_PORT: u16 = 0 as PortHandle;
const ORDERS_PORT: u16 = 1 as PortHandle;

#[derive(Debug)]
pub struct TestSourceFactory {
    running: Arc<AtomicBool>,
}

impl TestSourceFactory {
    pub fn new(running: Arc<AtomicBool>) -> Self {
        Self { running }
    }
}

impl SourceFactory<SchemaSQLContext> for TestSourceFactory {
    fn get_output_ports(&self) -> Result<Vec<OutputPortDef>, ExecutionError> {
        Ok(vec![
            OutputPortDef::new(
                SUPPLIERS_PORT,
                OutputPortType::StatefulWithPrimaryKeyLookup {
                    retr_old_records_for_updates: true,
                    retr_old_records_for_deletes: true,
                },
            ),
            OutputPortDef::new(
                ORDERS_PORT,
                OutputPortType::StatefulWithPrimaryKeyLookup {
                    retr_old_records_for_updates: true,
                    retr_old_records_for_deletes: true,
                },
            ),
        ])
    }

    fn get_output_schema(
        &self,
        port: &PortHandle,
    ) -> Result<(Schema, SchemaSQLContext), ExecutionError> {
        if port == &SUPPLIERS_PORT {
            let source_id = SourceDefinition::Dynamic;
            Ok((
                Schema::empty()
                    .field(
                        FieldDefinition::new(
                            String::from("supplier_id"),
                            FieldType::Int,
                            false,
                            source_id.clone(),
                        ),
                        true,
                    )
                    .field(
                        FieldDefinition::new(
                            String::from("supplier_name"),
                            FieldType::String,
                            false,
                            source_id,
                        ),
                        false,
                    )
                    .clone(),
                SchemaSQLContext::default(),
            ))
        } else if port == &ORDERS_PORT {
            let source_id = SourceDefinition::Dynamic;
            Ok((
                Schema::empty()
                    .field(
                        FieldDefinition::new(
                            String::from("order_id"),
                            FieldType::Int,
                            false,
                            source_id.clone(),
                        ),
                        true,
                    )
                    .field(
                        FieldDefinition::new(
                            String::from("order_date"),
                            FieldType::Date,
                            false,
                            source_id.clone(),
                        ),
                        false,
                    )
                    .field(
                        FieldDefinition::new(
                            String::from("supplier_id"),
                            FieldType::Int,
                            false,
                            source_id,
                        ),
                        false,
                    )
                    .clone(),
                SchemaSQLContext::default(),
            ))
        } else {
            panic!("Invalid Port Handle {port}");
        }
    }

    fn build(
        &self,
        _output_schemas: HashMap<PortHandle, Schema>,
    ) -> Result<Box<dyn Source>, ExecutionError> {
        Ok(Box::new(TestSource {
            running: self.running.clone(),
        }))
    }
}

#[derive(Debug)]
pub struct TestSource {
    running: Arc<AtomicBool>,
}

impl Source for TestSource {
    fn can_start_from(&self, _last_checkpoint: (u64, u64)) -> Result<bool, ExecutionError> {
        Ok(false)
    }

    fn start(
        &self,
        fw: &mut dyn SourceChannelForwarder,
        _from_seq: Option<(u64, u64)>,
    ) -> Result<(), ExecutionError> {
        let operations = vec![
            (
                Operation::Insert {
                    new: Record::new(
                        None,
                        vec![Field::Int(1000), Field::String("Microsoft".to_string())],
                        Some(1),
                    ),
                },
                SUPPLIERS_PORT,
            ),
            (
                Operation::Insert {
                    new: Record::new(
                        None,
                        vec![Field::Int(2000), Field::String("Oracle".to_string())],
                        Some(1),
                    ),
                },
                SUPPLIERS_PORT,
            ),
            (
                Operation::Insert {
                    new: Record::new(
                        None,
                        vec![Field::Int(3000), Field::String("Apple".to_string())],
                        Some(1),
                    ),
                },
                SUPPLIERS_PORT,
            ),
            (
                Operation::Insert {
                    new: Record::new(
                        None,
                        vec![Field::Int(4000), Field::String("Samsung".to_string())],
                        Some(1),
                    ),
                },
                SUPPLIERS_PORT,
            ),
            (
                Operation::Insert {
                    new: Record::new(
                        None,
                        vec![
                            Field::Int(1),
                            Field::Date(NaiveDate::from_ymd_opt(2015, 8, 1).unwrap()),
                            Field::Int(2000),
                        ],
                        Some(1),
                    ),
                },
                ORDERS_PORT,
            ),
            (
                Operation::Insert {
                    new: Record::new(
                        None,
                        vec![
                            Field::Int(2),
                            Field::Date(NaiveDate::from_ymd_opt(2015, 8, 1).unwrap()),
                            Field::Int(6000),
                        ],
                        Some(1),
                    ),
                },
                ORDERS_PORT,
            ),
            (
                Operation::Insert {
                    new: Record::new(
                        None,
                        vec![
                            Field::Int(3),
                            Field::Date(NaiveDate::from_ymd_opt(2015, 8, 2).unwrap()),
                            Field::Int(7000),
                        ],
                        Some(1),
                    ),
                },
                ORDERS_PORT,
            ),
            (
                Operation::Insert {
                    new: Record::new(
                        None,
                        vec![
                            Field::Int(4),
                            Field::Date(NaiveDate::from_ymd_opt(2015, 8, 3).unwrap()),
                            Field::Int(8000),
                        ],
                        Some(1),
                    ),
                },
                ORDERS_PORT,
            ),
        ];

        for (index, (op, port)) in operations.into_iter().enumerate() {
            fw.send(IngestionMessage::new_op(index as u64, 0, op), port)
                .unwrap();
        }

        loop {
            if !self.running.load(Ordering::Relaxed) {
                break;
            }
            // thread::sleep(Duration::from_millis(500));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct TestSinkFactory {
    expected: u64,
    running: Arc<AtomicBool>,
}

impl TestSinkFactory {
    pub fn new(expected: u64, barrier: Arc<AtomicBool>) -> Self {
        Self {
            expected,
            running: barrier,
        }
    }
}

impl SinkFactory<SchemaSQLContext> for TestSinkFactory {
    fn get_input_ports(&self) -> Vec<PortHandle> {
        vec![DEFAULT_PORT_HANDLE]
    }

    fn build(
        &self,
        _input_schemas: HashMap<PortHandle, Schema>,
        _source_states: &SourceStates,
    ) -> Result<Box<dyn Sink>, ExecutionError> {
        Ok(Box::new(TestSink {
            expected: self.expected,
            current: 0,
            running: self.running.clone(),
        }))
    }

    fn prepare(
        &self,
        _input_schemas: HashMap<PortHandle, (Schema, SchemaSQLContext)>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct TestSink {
    expected: u64,
    current: u64,
    running: Arc<AtomicBool>,
}

impl Sink for TestSink {
    fn process(
        &mut self,
        _from_port: PortHandle,
        _op: Operation,
        _state: &SharedTransaction,
        _reader: &HashMap<PortHandle, Box<dyn RecordReader>>,
    ) -> Result<(), ExecutionError> {
        match _op {
            Operation::Delete { old } => debug!("o0:-> - {:?}", old.values),
            Operation::Insert { new } => debug!("o0:-> + {:?}", new.values),
            Operation::Update { old, new } => {
                debug!("o0:-> - {:?}, + {:?}", old.values, new.values)
            }
        }

        self.current += 1;
        if self.current == self.expected {
            debug!(
                "Received {} messages. Notifying sender to exit!",
                self.current
            );
            self.running.store(false, Ordering::Relaxed);
        }
        Ok(())
    }

    fn commit(&mut self, _epoch: &Epoch, _tx: &SharedTransaction) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn on_source_snapshotting_done(&mut self) -> Result<(), ExecutionError> {
        Ok(())
    }
}
