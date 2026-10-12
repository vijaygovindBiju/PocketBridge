use crate::adapter::{execute_chunk_transform, ChunkTransformParams, TransformOutput};

pub struct LocalExecutor;

impl LocalExecutor {
    pub fn run_transform(params: &ChunkTransformParams) -> TransformOutput {
        execute_chunk_transform(params)
    }
}
