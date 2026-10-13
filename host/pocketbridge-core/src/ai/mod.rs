pub mod pipeline;
pub mod speculative;

pub use pipeline::{ActivationTensorPayload, ModelPipelineConfig, AI_PIPELINE_ADAPTER_ID};
pub use speculative::{SpeculativeCoordinator, SpeculativeRoundResult};
