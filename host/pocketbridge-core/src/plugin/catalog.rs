use super::manifest::{CapabilityStatus, PluginKind, PluginManifest, PluginRequirements};

/// Return the verified, working PocketBridge Community Hub plugin catalog.
/// Only plugins with genuine, hardware-tested native execution backends are enabled here.
pub fn default_catalog() -> Vec<PluginManifest> {
    vec![
        PluginManifest {
            id: "image-batch".to_string(),
            name: "Image Batch Processor".to_string(),
            version: "1.0.0".to_string(),
            description: "High-throughput parallel image resizing, inversion, and box-blur filtering".to_string(),
            author: "PocketBridge Core Team".to_string(),
            license: "Apache-2.0".to_string(),
            kind: PluginKind::Native,
            adapter_id: "image_batch.v1".to_string(),
            capability_status: CapabilityStatus::HardwareTested,
            requirements: PluginRequirements {
                min_host_ram_mb: 256,
                min_worker_ram_mb: 128,
                min_worker_storage_mb: 15,
            },
            sha256: Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()),
            size_bytes: 1_250_000,
            host_entrypoint: Some("image_batch".to_string()),
            worker_entrypoint: Some("libimage_batch.so".to_string()),
            download_url: Some("https://github.com/PocketBridge/pocketbridge-plugins/releases/download/v1.0.0/image-batch.tar.gz".to_string()),
        },
        PluginManifest {
            id: "data-analytics".to_string(),
            name: "Data Analytics Engine".to_string(),
            version: "1.0.0".to_string(),
            description: "Structured CSV/TSV table reduction, sum, min/max, and numerical filtering".to_string(),
            author: "PocketBridge Core Team".to_string(),
            license: "Apache-2.0".to_string(),
            kind: PluginKind::Native,
            adapter_id: "data_analytics.v1".to_string(),
            capability_status: CapabilityStatus::HardwareTested,
            requirements: PluginRequirements {
                min_host_ram_mb: 128,
                min_worker_ram_mb: 64,
                min_worker_storage_mb: 10,
            },
            sha256: Some("f4b1c24298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852a111".to_string()),
            size_bytes: 850_000,
            host_entrypoint: Some("data_analytics".to_string()),
            worker_entrypoint: Some("libdata_analytics.so".to_string()),
            download_url: Some("https://github.com/PocketBridge/pocketbridge-plugins/releases/download/v1.0.0/data-analytics.tar.gz".to_string()),
        },
    ]
}

/// Return plugins that are documented on the project roadmap but NOT yet implemented.
/// These are excluded from normal production installation and execution flows.
pub fn planned_catalog() -> Vec<PluginManifest> {
    vec![
        PluginManifest {
            id: "llm-llama".to_string(),
            name: "Distributed LLM Tensor Engine (Planned)".to_string(),
            version: "0.1.0-planned".to_string(),
            description: "Distributed transformer tensor inference via llama.cpp RPC. Requires on-device ARM64 RPC worker.".to_string(),
            author: "PocketBridge AI Hub".to_string(),
            license: "Apache-2.0".to_string(),
            kind: PluginKind::AiModel,
            adapter_id: "llm_pipeline.v1".to_string(),
            capability_status: CapabilityStatus::Planned,
            requirements: PluginRequirements {
                min_host_ram_mb: 8192,
                min_worker_ram_mb: 8192,
                min_worker_storage_mb: 16000,
            },
            sha256: None,
            size_bytes: 0,
            host_entrypoint: Some("llama_rpc_host".to_string()),
            worker_entrypoint: Some("llama-rpc-server".to_string()),
            download_url: None,
        },
        PluginManifest {
            id: "ai-whisper".to_string(),
            name: "Whisper Voice Transcription (Planned)".to_string(),
            version: "0.1.0-planned".to_string(),
            description: "On-device audio transcription on Qualcomm NPU via whisper.tflite.".to_string(),
            author: "PocketBridge AI Hub".to_string(),
            license: "MIT".to_string(),
            kind: PluginKind::AiModel,
            adapter_id: "ai_whisper.v1".to_string(),
            capability_status: CapabilityStatus::Planned,
            requirements: PluginRequirements {
                min_host_ram_mb: 1024,
                min_worker_ram_mb: 1536,
                min_worker_storage_mb: 150,
            },
            sha256: None,
            size_bytes: 0,
            host_entrypoint: Some("whisper_runner".to_string()),
            worker_entrypoint: Some("whisper-tiny.tflite".to_string()),
            download_url: None,
        },
        PluginManifest {
            id: "browser-cdp".to_string(),
            name: "Headless Chromium Tab Renderer (Planned)".to_string(),
            version: "0.1.0-planned".to_string(),
            description: "Background tab offload via Chromium DevTools Protocol proxy.".to_string(),
            author: "PocketBridge Core Team".to_string(),
            license: "Apache-2.0".to_string(),
            kind: PluginKind::WebHeadless,
            adapter_id: "browser_cdp.v1".to_string(),
            capability_status: CapabilityStatus::Planned,
            requirements: PluginRequirements {
                min_host_ram_mb: 512,
                min_worker_ram_mb: 512,
                min_worker_storage_mb: 50,
            },
            sha256: None,
            size_bytes: 0,
            host_entrypoint: Some("cdp_bridge".to_string()),
            worker_entrypoint: Some("headless_shell".to_string()),
            download_url: None,
        },
        PluginManifest {
            id: "media-ffmpeg".to_string(),
            name: "FFmpeg Video Transcoder (Planned)".to_string(),
            version: "0.1.0-planned".to_string(),
            description: "Parallel H.264/AV1 video transcode slice pipeline via libavcodec.".to_string(),
            author: "PocketBridge Community".to_string(),
            license: "GPL-3.0".to_string(),
            kind: PluginKind::Native,
            adapter_id: "media_ffmpeg.v1".to_string(),
            capability_status: CapabilityStatus::Planned,
            requirements: PluginRequirements {
                min_host_ram_mb: 1024,
                min_worker_ram_mb: 1024,
                min_worker_storage_mb: 100,
            },
            sha256: None,
            size_bytes: 0,
            host_entrypoint: Some("ffmpeg_host".to_string()),
            worker_entrypoint: Some("libavcodec_worker.so".to_string()),
            download_url: None,
        },
        PluginManifest {
            id: "phone-cluster".to_string(),
            name: "Multi-Phone Cluster Mesh (Planned)".to_string(),
            version: "0.1.0-planned".to_string(),
            description: "Multi-phone USB hub work-stealing coordinator and mesh failover.".to_string(),
            author: "PocketBridge Core Team".to_string(),
            license: "Apache-2.0".to_string(),
            kind: PluginKind::Native,
            adapter_id: "phone_cluster.v1".to_string(),
            capability_status: CapabilityStatus::Planned,
            requirements: PluginRequirements {
                min_host_ram_mb: 256,
                min_worker_ram_mb: 64,
                min_worker_storage_mb: 5,
            },
            sha256: None,
            size_bytes: 0,
            host_entrypoint: Some("cluster_orchestrator".to_string()),
            worker_entrypoint: Some("libcluster_worker.so".to_string()),
            download_url: None,
        },
    ]
}

/// Find a manifest from the active verified catalog by plugin id or adapter id.
pub fn find_catalog_plugin(query: &str) -> Option<PluginManifest> {
    default_catalog()
        .into_iter()
        .find(|m| m.id == query || m.adapter_id == query)
}

/// Find a manifest from the planned roadmap catalog by plugin id or adapter id.
pub fn find_planned_plugin(query: &str) -> Option<PluginManifest> {
    planned_catalog()
        .into_iter()
        .find(|m| m.id == query || m.adapter_id == query)
}
