use pocketbridge_core::{
    PartitionStrategy, UniversalPartitioner,
};

#[test]
fn test_universal_partition_lines() {
    let raw_text = (0..100)
        .map(|i| format!("line_{:03},metric_{}", i, i * 10))
        .collect::<Vec<_>>()
        .join("\n");

    let (host_slice, worker_slice) = UniversalPartitioner::partition_lines(&raw_text, 0.6);

    let host_lines: Vec<&str> = host_slice.lines().collect();
    let worker_lines: Vec<&str> = worker_slice.lines().collect();

    assert_eq!(host_lines.len(), 60);
    assert_eq!(worker_lines.len(), 40);
    assert_eq!(host_lines[0], "line_000,metric_0");
    assert_eq!(worker_lines[0], "line_060,metric_600");
}

#[test]
fn test_universal_partition_bytes() {
    let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
    let (host_bytes, worker_bytes) = UniversalPartitioner::partition_bytes(&data, 0.75);

    assert_eq!(host_bytes.len(), 750);
    assert_eq!(worker_bytes.len(), 250);
    assert_eq!(host_bytes.len() + worker_bytes.len(), 1000);
}

#[test]
fn test_universal_partition_items() {
    let files = vec![
        "video_01.mp4", "video_02.mp4", "video_03.mp4", "video_04.mp4",
        "video_05.mp4", "video_06.mp4", "video_07.mp4", "video_08.mp4",
        "video_09.mp4", "video_10.mp4",
    ];

    let (host_files, worker_files) = UniversalPartitioner::partition_items(&files, 0.5);

    assert_eq!(host_files.len(), 5);
    assert_eq!(worker_files.len(), 5);
    assert_eq!(host_files[0], "video_01.mp4");
    assert_eq!(worker_files[0], "video_06.mp4");
}

#[test]
fn test_universal_plan_clamping() {
    let plan_low = UniversalPartitioner::plan(PartitionStrategy::LineRecord, 100, 0.001);
    assert_eq!(plan_low.host_ratio, 0.01);
    assert_eq!(plan_low.host_units, 1);
    assert_eq!(plan_low.worker_units, 99);

    let plan_high = UniversalPartitioner::plan(PartitionStrategy::ByteChunk, 100, 1.5);
    assert_eq!(plan_high.host_ratio, 0.99);
    assert_eq!(plan_high.host_units, 99);
    assert_eq!(plan_high.worker_units, 1);
}
