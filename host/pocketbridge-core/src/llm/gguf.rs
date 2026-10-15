use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::error::PocketBridgeError;

/// GGUF Magic number: 'GGUF' in ASCII = 0x46554747 (little-endian)
pub const GGUF_MAGIC: u32 = 0x46554747;

/// Supported GGML types in GGUF
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GgmlType {
    F32 = 0,
    F16 = 1,
    Q4_0 = 2,
    Q4_1 = 3,
    Q5_0 = 6,
    Q5_1 = 7,
    Q8_0 = 8,
    Q8_1 = 9,
    Q2_K = 10,
    Q3_K = 11,
    Q4_K = 12,
    Q5_K = 13,
    Q6_K = 14,
    Q8_K = 15,
    IQ2_XXS = 16,
    IQ2_XS = 17,
    IQ3_XXS = 18,
    IQ1_S = 19,
    IQ4_NL = 20,
    IQ3_S = 21,
    IQ2_S = 22,
    IQ4_XS = 23,
    I8 = 24,
    I16 = 25,
    I32 = 26,
    I64 = 27,
    F64 = 28,
    IQ1_M = 29,
    BF16 = 30,
    Unknown = 999,
}

impl From<u32> for GgmlType {
    fn from(val: u32) -> Self {
        match val {
            0 => GgmlType::F32,
            1 => GgmlType::F16,
            2 => GgmlType::Q4_0,
            3 => GgmlType::Q4_1,
            6 => GgmlType::Q5_0,
            7 => GgmlType::Q5_1,
            8 => GgmlType::Q8_0,
            9 => GgmlType::Q8_1,
            10 => GgmlType::Q2_K,
            11 => GgmlType::Q3_K,
            12 => GgmlType::Q4_K,
            13 => GgmlType::Q5_K,
            14 => GgmlType::Q6_K,
            15 => GgmlType::Q8_K,
            24 => GgmlType::I8,
            25 => GgmlType::I16,
            26 => GgmlType::I32,
            27 => GgmlType::I64,
            28 => GgmlType::F64,
            30 => GgmlType::BF16,
            _ => GgmlType::Unknown,
        }
    }
}

/// Metadata value parsed from GGUF key-value store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GgufValue {
    Uint8(u8),
    Int8(i8),
    Uint16(u16),
    Int16(i16),
    Uint32(u32),
    Int32(i32),
    Float32(f32),
    Bool(bool),
    String(String),
    ArrayString(Vec<String>),
    ArrayUint64(Vec<u64>),
    ArrayInt64(Vec<i64>),
    ArrayFloat32(Vec<f32>),
    Uint64(u64),
    Int64(i64),
    Float64(f64),
    Other(String),
}

/// Metadata and layout descriptors for a GGUF model tensor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GgufTensorInfo {
    pub name: String,
    pub n_dims: u32,
    pub dimensions: Vec<u64>,
    pub ggml_type: GgmlType,
    pub offset: u64,
    pub estimated_bytes: u64,
}

/// Parsed GGUF Model Header and Tensor Catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GgufModel {
    pub file_path: PathBuf,
    pub file_size_bytes: u64,
    pub version: u32,
    pub tensor_count: u64,
    pub metadata_kv_count: u64,
    pub architecture: String,
    pub model_name: String,
    pub block_count: usize,
    pub context_length: u64,
    pub embedding_length: u64,
    pub feed_forward_length: u64,
    pub attention_head_count: u64,
    pub attention_head_count_kv: u64,
    pub tensors: Vec<GgufTensorInfo>,
}

impl GgufModel {
    /// Open and parse the header, metadata, and tensor descriptors of a GGUF file.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, PocketBridgeError> {
        let path_buf = path.as_ref().to_path_buf();
        let file = File::open(&path_buf)?;

        let file_size_bytes = file.metadata().map(|m| m.len()).unwrap_or(0);
        let mut reader = BufReader::new(file);

        // 1. Magic
        let mut magic_buf = [0u8; 4];
        reader.read_exact(&mut magic_buf).map_err(|e| {
            PocketBridgeError::ModelError(format!("Cannot read GGUF magic: {}", e))
        })?;
        let magic = u32::from_le_bytes(magic_buf);
        if magic != GGUF_MAGIC {
            return Err(PocketBridgeError::ModelError(format!(
                "Invalid GGUF magic 0x{:08X} (expected 0x{:08X} 'GGUF')",
                magic, GGUF_MAGIC
            )));
        }

        // 2. Version
        let mut ver_buf = [0u8; 4];
        reader.read_exact(&mut ver_buf).map_err(|e| {
            PocketBridgeError::ModelError(format!("Cannot read GGUF version: {}", e))
        })?;
        let version = u32::from_le_bytes(ver_buf);

        // 3. Counts
        let mut t_count_buf = [0u8; 8];
        reader.read_exact(&mut t_count_buf).map_err(|e| {
            PocketBridgeError::ModelError(format!("Cannot read tensor count: {}", e))
        })?;
        let tensor_count = u64::from_le_bytes(t_count_buf);

        let mut kv_count_buf = [0u8; 8];
        reader.read_exact(&mut kv_count_buf).map_err(|e| {
            PocketBridgeError::ModelError(format!("Cannot read metadata count: {}", e))
        })?;
        let metadata_kv_count = u64::from_le_bytes(kv_count_buf);

        let mut architecture = String::from("unknown");
        let mut model_name = String::from("unnamed");
        let mut block_count: usize = 0;
        let mut context_length: u64 = 0;
        let mut embedding_length: u64 = 0;
        let mut feed_forward_length: u64 = 0;
        let mut attention_head_count: u64 = 0;
        let mut attention_head_count_kv: u64 = 0;

        // Helper to read length-prefixed string
        let read_str = |r: &mut BufReader<File>| -> Result<String, PocketBridgeError> {
            let mut len_buf = [0u8; 8];
            r.read_exact(&mut len_buf).map_err(|e| {
                PocketBridgeError::ModelError(format!("Failed reading string length: {}", e))
            })?;
            let len = u64::from_le_bytes(len_buf) as usize;
            let mut s_buf = vec![0u8; len];
            r.read_exact(&mut s_buf).map_err(|e| {
                PocketBridgeError::ModelError(format!("Failed reading string content: {}", e))
            })?;
            Ok(String::from_utf8_lossy(&s_buf).to_string())
        };

        // 4. Read Metadata KV pairs
        for _ in 0..metadata_kv_count {
            let key = read_str(&mut reader)?;
            let mut type_buf = [0u8; 4];
            reader.read_exact(&mut type_buf).map_err(|e| {
                PocketBridgeError::ModelError(format!("Failed reading value type: {}", e))
            })?;
            let val_type = u32::from_le_bytes(type_buf);

            match val_type {
                0 | 1 | 7 => {
                    let mut b = [0u8; 1];
                    reader.read_exact(&mut b).map_err(|e| PocketBridgeError::ModelError(e.to_string()))?;
                }
                2 | 3 => {
                    let mut b = [0u8; 2];
                    reader.read_exact(&mut b).map_err(|e| PocketBridgeError::ModelError(e.to_string()))?;
                }
                4 => {
                    let mut b = [0u8; 4];
                    reader.read_exact(&mut b).map_err(|e| PocketBridgeError::ModelError(e.to_string()))?;
                    let val = u32::from_le_bytes(b);
                    if key.ends_with(".block_count") { block_count = val as usize; }
                    else if key.ends_with(".context_length") { context_length = val as u64; }
                    else if key.ends_with(".embedding_length") { embedding_length = val as u64; }
                    else if key.ends_with(".feed_forward_length") { feed_forward_length = val as u64; }
                    else if key.ends_with(".head_count") { attention_head_count = val as u64; }
                    else if key.ends_with(".head_count_kv") { attention_head_count_kv = val as u64; }
                }
                5 | 6 => {
                    let mut b = [0u8; 4];
                    reader.read_exact(&mut b).map_err(|e| PocketBridgeError::ModelError(e.to_string()))?;
                }
                8 => {
                    let val = read_str(&mut reader)?;
                    if key == "general.architecture" { architecture = val.clone(); }
                    else if key == "general.name" { model_name = val.clone(); }
                }
                9 => {
                    // Array
                    let mut arr_type_buf = [0u8; 4];
                    reader.read_exact(&mut arr_type_buf).map_err(|e| PocketBridgeError::ModelError(e.to_string()))?;
                    let arr_type = u32::from_le_bytes(arr_type_buf);
                    let mut arr_len_buf = [0u8; 8];
                    reader.read_exact(&mut arr_len_buf).map_err(|e| PocketBridgeError::ModelError(e.to_string()))?;
                    let arr_len = u64::from_le_bytes(arr_len_buf) as usize;

                    let elem_size = match arr_type {
                        0 | 1 | 7 => 1,
                        2 | 3 => 2,
                        4 | 5 | 6 => 4,
                        10 | 11 | 12 => 8,
                        8 => 0, // variable strings
                        _ => 1,
                    };
                    if elem_size > 0 {
                        let total_skip = (arr_len as u64) * elem_size;
                        reader.seek(SeekFrom::Current(total_skip as i64)).map_err(|e| {
                            PocketBridgeError::ModelError(e.to_string())
                        })?;
                    } else if arr_type == 8 {
                        // Skip array of strings
                        for _ in 0..arr_len {
                            let _ = read_str(&mut reader)?;
                        }
                    }
                }
                10 => {
                    let mut b = [0u8; 8];
                    reader.read_exact(&mut b).map_err(|e| PocketBridgeError::ModelError(e.to_string()))?;
                    let val = u64::from_le_bytes(b);
                    if key.ends_with(".block_count") { block_count = val as usize; }
                    else if key.ends_with(".context_length") { context_length = val; }
                    else if key.ends_with(".embedding_length") { embedding_length = val; }
                    else if key.ends_with(".feed_forward_length") { feed_forward_length = val; }
                    else if key.ends_with(".head_count") { attention_head_count = val; }
                    else if key.ends_with(".head_count_kv") { attention_head_count_kv = val; }
                }
                11 | 12 => {
                    let mut b = [0u8; 8];
                    reader.read_exact(&mut b).map_err(|e| PocketBridgeError::ModelError(e.to_string()))?;
                }
                _ => {}
            }
        }

        // 5. Read Tensor Descriptors
        let mut tensors = Vec::with_capacity(tensor_count as usize);
        for _ in 0..tensor_count {
            let name = read_str(&mut reader)?;
            let mut dims_count_buf = [0u8; 4];
            reader.read_exact(&mut dims_count_buf).map_err(|e| {
                PocketBridgeError::ModelError(e.to_string())
            })?;
            let n_dims = u32::from_le_bytes(dims_count_buf);

            let mut dimensions = Vec::with_capacity(n_dims as usize);
            let mut element_count: u64 = 1;
            for _ in 0..n_dims {
                let mut d_buf = [0u8; 8];
                reader.read_exact(&mut d_buf).map_err(|e| {
                    PocketBridgeError::ModelError(e.to_string())
                })?;
                let dim = u64::from_le_bytes(d_buf);
                element_count = element_count.saturating_mul(dim);
                dimensions.push(dim);
            }

            let mut t_type_buf = [0u8; 4];
            reader.read_exact(&mut t_type_buf).map_err(|e| {
                PocketBridgeError::ModelError(e.to_string())
            })?;
            let ggml_type: GgmlType = u32::from_le_bytes(t_type_buf).into();

            let mut offset_buf = [0u8; 8];
            reader.read_exact(&mut offset_buf).map_err(|e| {
                PocketBridgeError::ModelError(e.to_string())
            })?;
            let offset = u64::from_le_bytes(offset_buf);

            // Estimate tensor byte size based on quantization
            let bytes_per_element_num = match ggml_type {
                GgmlType::F32 => 4.0,
                GgmlType::F16 | GgmlType::BF16 => 2.0,
                GgmlType::Q8_0 | GgmlType::Q8_1 => 34.0 / 32.0, // 1.0625
                GgmlType::Q8_K => 288.0 / 256.0,                // 1.125
                GgmlType::Q6_K => 210.0 / 256.0,                // 0.8203125
                GgmlType::Q5_0 => 22.0 / 32.0,                  // 0.6875
                GgmlType::Q5_1 => 24.0 / 32.0,                  // 0.75
                GgmlType::Q5_K => 176.0 / 256.0,                // 0.6875
                GgmlType::Q4_0 => 18.0 / 32.0,                  // 0.5625
                GgmlType::Q4_1 => 20.0 / 32.0,                  // 0.625
                GgmlType::Q4_K => 144.0 / 256.0,                // 0.5625
                GgmlType::Q3_K => 110.0 / 256.0,                // 0.4296875
                GgmlType::Q2_K => 84.0 / 256.0,                 // 0.328125
                _ => 1.0,
            };
            let estimated_bytes = ((element_count as f64) * bytes_per_element_num).round() as u64;

            tensors.push(GgufTensorInfo {
                name,
                n_dims,
                dimensions,
                ggml_type,
                offset,
                estimated_bytes,
            });
        }

        Ok(Self {
            file_path: path_buf,
            file_size_bytes,
            version,
            tensor_count,
            metadata_kv_count,
            architecture,
            model_name,
            block_count,
            context_length,
            embedding_length,
            feed_forward_length,
            attention_head_count,
            attention_head_count_kv,
            tensors,
        })
    }

    /// Calculate the memory footprint of the KV cache in bytes for a given context length and layer count.
    /// Each layer stores Key and Value tensors: 2 * n_head_kv * head_dim * sizeof(fp16) per token.
    pub fn kv_cache_bytes(&self, context_tokens: u64, layers: usize) -> u64 {
        let head_dim = if self.attention_head_count > 0 {
            self.embedding_length / self.attention_head_count
        } else {
            128
        };
        let kv_heads = if self.attention_head_count_kv > 0 {
            self.attention_head_count_kv
        } else {
            self.attention_head_count.max(1)
        };
        // 2 (K and V) * kv_heads * head_dim * 2 (FP16 bytes)
        let bytes_per_token_per_layer = 2 * kv_heads * head_dim * 2;
        context_tokens * (layers as u64) * bytes_per_token_per_layer
    }
}
