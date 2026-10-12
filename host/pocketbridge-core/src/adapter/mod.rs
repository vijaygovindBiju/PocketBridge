pub mod chunk_adapter;
pub mod chunk_transform;
pub mod data_analytics;
pub mod image_batch;
pub mod registry;
pub mod traits;
pub mod types;

pub use chunk_adapter::ChunkTransformAdapter;
pub use chunk_transform::{execute_chunk_transform, transform_record, ADAPTER_ID};
pub use data_analytics::{
    execute_analytics, DataAnalyticsAdapter, DataAnalyticsParams, DATA_ANALYTICS_ADAPTER_ID,
};
pub use image_batch::{
    execute_image_batch, process_single_image, ImageBatchAdapter, ImageBatchParams,
    IMAGE_BATCH_ADAPTER_ID,
};
pub use registry::AdapterRegistry;
pub use traits::{AdapterExecutionResult, WorkloadAdapter};
pub use types::{ChunkTransformParams, TransformOutput};
