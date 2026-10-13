use pocketbridge_core::{
    BrowserRendererRouter, BrowserTabDescriptor, ModelPipelineConfig,
    SpeculativeCoordinator, TabExecutionTarget,
};

#[test]
fn test_browser_tab_router_classification() {
    let tabs = vec![
        BrowserTabDescriptor {
            tab_id: 1,
            title: "GitHub - PocketBridge".into(),
            url: "https://github.com/vijaygovindBiju/PocketBridge".into(),
            is_active: true,
            is_audible: false,
            memory_estimate_bytes: 120 * 1024 * 1024, // 120 MB
        },
        BrowserTabDescriptor {
            tab_id: 2,
            title: "YouTube Music".into(),
            url: "https://music.youtube.com".into(),
            is_active: false,
            is_audible: true,
            memory_estimate_bytes: 350 * 1024 * 1024, // 350 MB
        },
        BrowserTabDescriptor {
            tab_id: 3,
            title: "Documentation".into(),
            url: "https://doc.rust-lang.org".into(),
            is_active: false,
            is_audible: false,
            memory_estimate_bytes: 80 * 1024 * 1024, // 80 MB
        },
    ];

    let (host_tabs, worker_tabs) = BrowserRendererRouter::route_tabs(&tabs);

    assert_eq!(host_tabs.len(), 1);
    assert_eq!(host_tabs[0].tab_id, 1);
    assert_eq!(
        BrowserRendererRouter::classify_tab(&host_tabs[0]),
        TabExecutionTarget::HostActive
    );

    assert_eq!(worker_tabs.len(), 2);
    assert_eq!(worker_tabs[0].tab_id, 2);
    assert_eq!(worker_tabs[1].tab_id, 3);

    let (saved_bytes, saved_pct) = BrowserRendererRouter::calculate_ram_savings(&host_tabs, &worker_tabs);
    assert_eq!(saved_bytes, (350 + 80) * 1024 * 1024);
    assert!(saved_pct > 70.0); // Over 70% of memory offloaded to phone
}

#[test]
fn test_ai_pipeline_layer_partitioning() {
    // 32-layer LLaMA model with hidden dimension 4096
    let config = ModelPipelineConfig::new("llama-3-8b", 32, 4096, 0.5);

    assert_eq!(config.total_layers, 32);
    assert_eq!(config.host_layers, 16);
    assert_eq!(config.worker_layers, 16);
    // Float16 activation tensor size: 4096 * 2 = 8192 bytes
    assert_eq!(config.activation_tensor_bytes(), 8192);

    // Latency over USB (372 Mbps) is < 0.2 ms
    let latency = config.transfer_latency_ms(372.0);
    assert!(latency < 0.20);
}

#[test]
fn test_ai_speculative_decoding_speedup() {
    // Small model drafts 5 tokens in 20 ms
    // Big model verifies in 15 ms
    // Baseline big model takes 25 ms per token (5 * 25 = 125 ms)
    let res = SpeculativeCoordinator::evaluate_speedup(5, 5, 20.0, 15.0, 25.0);

    assert_eq!(res.draft_tokens_count, 5);
    assert_eq!(res.accepted_tokens_count, 5);
    // 125 ms / 35 ms ~ 3.57x speedup
    assert!(res.effective_speedup > 3.0);
}
