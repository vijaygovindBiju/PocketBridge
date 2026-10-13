pub mod interceptor;
pub mod partitioner;

pub use interceptor::{UniversalExecPayload, UniversalExecResult, UNIVERSAL_ADAPTER_ID};
pub use partitioner::{PartitionStrategy, UniversalPartitionPlan, UniversalPartitioner};
