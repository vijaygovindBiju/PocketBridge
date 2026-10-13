pub mod adapter;
pub mod benchmark;
pub mod coprocessor;
pub mod error;
pub mod fleet;
pub mod job;
pub mod profiler;
pub mod protocol;
pub mod ai;
pub mod browser;
pub mod plugin;
pub mod scheduler;
pub mod transport;
pub mod universal;

pub use plugin::{
    default_catalog, find_catalog_plugin, DynamicPluginAdapter, PluginKind, PluginManifest,
    PluginManager, PluginRequirements,
};

pub use ai::{
    ActivationTensorPayload, ModelPipelineConfig, SpeculativeCoordinator, SpeculativeRoundResult,
    AI_PIPELINE_ADAPTER_ID,
};
pub use browser::{
    BrowserRendererRouter, BrowserTabDescriptor, TabExecutionTarget, TabRenderPayload,
    TabRenderStatus, BROWSER_ADAPTER_ID,
};
pub use universal::{
    PartitionStrategy, UniversalExecPayload, UniversalExecResult, UniversalPartitionPlan,
    UniversalPartitioner, UNIVERSAL_ADAPTER_ID,
};

pub use adapter::{
    execute_chunk_transform, transform_record, AdapterExecutionResult, AdapterRegistry,
    ChunkTransformParams, DataAnalyticsParams, ImageBatchParams, TransformOutput,
    WorkloadAdapter, ADAPTER_ID, DATA_ANALYTICS_ADAPTER_ID, IMAGE_BATCH_ADAPTER_ID,
};
pub use coprocessor::{partition_workload, CoProcessingResult, CoProcessor, PartitionConfig};
pub use fleet::{
    ChunkResult, FleetCoordinator, FleetExecutionSummary, FleetHealthMonitor, FleetManager,
    FleetWorkQueue, WorkChunk, WorkerNode, WorkerNodeState,
};
pub use benchmark::{
    calculate_stats, format_bytes, format_summary_table, run_benchmark_suite, run_single_echo,
    save_benchmark_report, BenchmarkConfig, BenchmarkReport, BenchmarkStats, PayloadSizeResult,
    SampleResult, TestType,
};
pub use error::PocketBridgeError;
pub use job::{JobExecutionResult, JobState, LocalExecutor, RemoteJobClient};
pub use profiler::{collect_host_profile, DeviceProfile};
pub use protocol::{
    drain_raw_stream, read_envelope, write_envelope, write_raw_stream, AgentRole,
    BenchmarkEchoPayload, BenchmarkEchoReplyPayload, CancelJobPayload, Envelope, HeartbeatPayload,
    HelloPayload, JobAcceptedPayload, JobCancelledPayload, JobErrorPayload, JobLimits,
    JobProgressPayload, JobRejectedPayload, JobRequirements, JobStartedPayload, MessageKind,
    ResultManifestPayload, SubmitJobPayload,
};
pub use scheduler::{
    ConstraintEvaluation, CostEstimate, CostModel, ExecutionTarget, PerformanceBaseline,
    PolicyEngine, Scheduler, SchedulerConfig, SchedulingDecision,
};
pub use transport::{AdbTransport, DEFAULT_PORT};
