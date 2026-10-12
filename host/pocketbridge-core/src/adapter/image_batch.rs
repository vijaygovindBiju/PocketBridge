use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Instant;
use super::traits::{AdapterExecutionResult, WorkloadAdapter};
use crate::error::PocketBridgeError;

pub const IMAGE_BATCH_ADAPTER_ID: &str = "image_batch.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageBatchParams {
    pub image_count: u32,
    pub width: u32,
    pub height: u32,
    pub seed: u64,
    pub operation: String, // "grayscale_invert", "box_blur_3x3", "downscale_half"
}

impl Default for ImageBatchParams {
    fn default() -> Self {
        Self {
            image_count: 50,
            width: 256,
            height: 256,
            seed: 0xCAFEBABEDEADBEEF,
            operation: "grayscale_invert".to_string(),
        }
    }
}

pub struct ImageBatchAdapter;

impl WorkloadAdapter for ImageBatchAdapter {
    fn adapter_id(&self) -> &'static str {
        IMAGE_BATCH_ADAPTER_ID
    }

    fn description(&self) -> &'static str {
        "Batch parallel image processing (color space conversion, spatial convolution, and downscaling)"
    }

    fn validate_params(&self, params: &serde_json::Value) -> Result<(), PocketBridgeError> {
        let parsed: ImageBatchParams = serde_json::from_value(params.clone())
            .map_err(|e| PocketBridgeError::AdapterError(format!("Invalid ImageBatchParams: {e}")))?;
        if parsed.image_count == 0 || parsed.width == 0 || parsed.height == 0 {
            return Err(PocketBridgeError::AdapterError("Image dimensions and count must be positive".into()));
        }
        match parsed.operation.as_str() {
            "grayscale_invert" | "box_blur_3x3" | "downscale_half" => Ok(()),
            other => Err(PocketBridgeError::AdapterError(format!("Unsupported image operation: {other}"))),
        }
    }

    fn estimate_payload_sizes(&self, params: &serde_json::Value) -> Result<(usize, usize), PocketBridgeError> {
        let parsed: ImageBatchParams = serde_json::from_value(params.clone())
            .map_err(|e| PocketBridgeError::AdapterError(format!("Invalid ImageBatchParams: {e}")))?;
        let in_size = (parsed.image_count as usize) * (parsed.width as usize) * (parsed.height as usize) * 3;
        let out_size = match parsed.operation.as_str() {
            "downscale_half" => (parsed.image_count as usize) * ((parsed.width / 2) as usize) * ((parsed.height / 2) as usize) * 3,
            _ => in_size,
        };
        Ok((in_size, out_size))
    }

    fn execute_local(&self, params: &serde_json::Value) -> Result<AdapterExecutionResult, PocketBridgeError> {
        let parsed: ImageBatchParams = serde_json::from_value(params.clone())
            .map_err(|e| PocketBridgeError::AdapterError(format!("Invalid ImageBatchParams: {e}")))?;

        let t_start = Instant::now();
        let (output_sha256, bytes_out) = execute_image_batch(&parsed);
        let duration_ms = t_start.elapsed().as_secs_f64() * 1000.0;

        Ok(AdapterExecutionResult {
            adapter_id: IMAGE_BATCH_ADAPTER_ID.to_string(),
            status: "succeeded".to_string(),
            duration_ms,
            bytes_processed: bytes_out,
            output_sha256,
            summary: format!(
                "Transformed {} images ({}x{}, op: {}) in {:.2} ms",
                parsed.image_count, parsed.width, parsed.height, parsed.operation, duration_ms
            ),
            metadata: Some(serde_json::json!({
                "image_count": parsed.image_count,
                "width": parsed.width,
                "height": parsed.height,
                "operation": parsed.operation,
            })),
        })
    }
}

/// Execute image batch transformation locally across host CPU threads.
pub fn execute_image_batch(params: &ImageBatchParams) -> (String, usize) {
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let images_per_thread = ((params.image_count as usize) + num_threads - 1) / num_threads;
    let mut thread_buffers = vec![Vec::new(); num_threads];

    std::thread::scope(|s| {
        for (t_idx, buf_slot) in thread_buffers.iter_mut().enumerate() {
            let start_img = (t_idx * images_per_thread) as u32;
            let end_img = (((t_idx + 1) * images_per_thread) as u32).min(params.image_count);
            if start_img >= end_img {
                continue;
            }

            s.spawn(move || {
                let mut local_buf = Vec::new();
                for img_idx in start_img..end_img {
                    process_single_image(params, img_idx, &mut local_buf);
                }
                *buf_slot = local_buf;
            });
        }
    });

    let mut hasher = Sha256::new();
    let mut total_bytes = 0;
    for buf in thread_buffers {
        total_bytes += buf.len();
        hasher.update(&buf);
    }

    (format!("{:x}", hasher.finalize()), total_bytes)
}

/// Deterministically generates and processes a single image according to parameters.
pub fn process_single_image(params: &ImageBatchParams, img_idx: u32, out_buf: &mut Vec<u8>) {
    let w = params.width;
    let h = params.height;

    // 1. Generate deterministic raw RGB pixels
    let mut raw_pixels = vec![0u8; (w * h * 3) as usize];
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 3) as usize;
            raw_pixels[idx] = (((x * 131) ^ (y * 197) ^ (img_idx * 283) ^ (params.seed as u32)) % 256) as u8;
            raw_pixels[idx + 1] = (((x * 173) ^ (y * 151) ^ (img_idx * 311) ^ ((params.seed >> 8) as u32)) % 256) as u8;
            raw_pixels[idx + 2] = (((x * 199) ^ (y * 223) ^ (img_idx * 359) ^ ((params.seed >> 16) as u32)) % 256) as u8;
        }
    }

    // 2. Apply requested operation
    match params.operation.as_str() {
        "grayscale_invert" => {
            for i in 0..(w * h) as usize {
                let idx = i * 3;
                let r = raw_pixels[idx] as u32;
                let g = raw_pixels[idx + 1] as u32;
                let b = raw_pixels[idx + 2] as u32;
                let lum = (299 * r + 587 * g + 114 * b) / 1000;
                let inv = (255 - lum) as u8;
                out_buf.push(inv);
                out_buf.push(inv);
                out_buf.push(inv);
            }
        }
        "downscale_half" => {
            let w_half = w / 2;
            let h_half = h / 2;
            for y in 0..h_half {
                for x in 0..w_half {
                    let mut sum_r = 0u32;
                    let mut sum_g = 0u32;
                    let mut sum_b = 0u32;
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let src_x = x * 2 + dx;
                            let src_y = y * 2 + dy;
                            let idx = ((src_y * w + src_x) * 3) as usize;
                            sum_r += raw_pixels[idx] as u32;
                            sum_g += raw_pixels[idx + 1] as u32;
                            sum_b += raw_pixels[idx + 2] as u32;
                        }
                    }
                    out_buf.push((sum_r / 4) as u8);
                    out_buf.push((sum_g / 4) as u8);
                    out_buf.push((sum_b / 4) as u8);
                }
            }
        }
        "box_blur_3x3" => {
            for y in 0..h as i32 {
                for x in 0..w as i32 {
                    let mut sum_r = 0u32;
                    let mut sum_g = 0u32;
                    let mut sum_b = 0u32;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let px = (x + dx).clamp(0, w as i32 - 1) as u32;
                            let py = (y + dy).clamp(0, h as i32 - 1) as u32;
                            let idx = ((py * w + px) * 3) as usize;
                            sum_r += raw_pixels[idx] as u32;
                            sum_g += raw_pixels[idx + 1] as u32;
                            sum_b += raw_pixels[idx + 2] as u32;
                        }
                    }
                    out_buf.push((sum_r / 9) as u8);
                    out_buf.push((sum_g / 9) as u8);
                    out_buf.push((sum_b / 9) as u8);
                }
            }
        }
        _ => {
            out_buf.extend_from_slice(&raw_pixels);
        }
    }
}
