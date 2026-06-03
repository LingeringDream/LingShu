use rand::Rng;
use std::time::Instant;
use uuid::Uuid;

const COLLECTION: &str = "test_memories";
const VECTOR_SIZE: usize = 768;
const NUM_VECTORS: usize = 100_000;
const NUM_QUERIES: usize = 1000;

#[tokio::main]
async fn main() {
    let base_url =
        std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".to_string());
    let client = reqwest::Client::new();

    println!("=== Qdrant Vector Search PoC ===\n");

    // 1. Create collection
    println!("Creating collection '{}'...", COLLECTION);
    let create_body = serde_json::json!({
        "vectors": {
            "size": VECTOR_SIZE,
            "distance": "Cosine"
        },
        "optimizers_config": {
            "indexing_threshold": 20000
        },
        "on_disk_payload": true
    });

    client
        .put(format!("{}/collections/{}", base_url, COLLECTION))
        .json(&create_body)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    println!("Collection created.\n");

    // 2. Bulk insert
    println!("Inserting {} vectors...", NUM_VECTORS);
    let insert_start = Instant::now();
    let mut rng = rand::thread_rng();
    let batch_size = 1000;

    for batch_idx in (0..NUM_VECTORS).step_by(batch_size) {
        let end = (batch_idx + batch_size).min(NUM_VECTORS);
        let points: Vec<serde_json::Value> = (batch_idx..end)
            .map(|_| {
                let vector: Vec<f32> = (0..VECTOR_SIZE).map(|_| rng.gen()).collect();
                serde_json::json!({
                    "id": Uuid::new_v4().to_string(),
                    "vector": vector,
                    "payload": {
                        "memory_type": if rng.gen_bool(0.5) { "episodic" } else { "semantic" },
                        "project_id": Uuid::new_v4().to_string(),
                        "importance": rng.gen_range(0.0..1.0)
                    }
                })
            })
            .collect();

        client
            .put(format!("{}/collections/{}/points", base_url, COLLECTION))
            .json(&serde_json::json!({ "points": points }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();

        if (batch_idx + batch_size) % 10000 == 0 {
            print!("  {}...", batch_idx + batch_size);
        }
    }
    let insert_time = insert_start.elapsed();
    println!("\nInsert done in {:.2}s\n", insert_time.as_secs_f64());

    // 3. Search latency test
    println!("Running {} search queries...", NUM_QUERIES);
    let mut latencies = Vec::with_capacity(NUM_QUERIES);

    for _ in 0..NUM_QUERIES {
        let query_vector: Vec<f32> = (0..VECTOR_SIZE).map(|_| rng.gen()).collect();

        let start = Instant::now();
        let search_body = serde_json::json!({
            "vector": query_vector,
            "limit": 10,
            "with_payload": true
        });

        client
            .post(format!(
                "{}/collections/{}/points/search",
                base_url, COLLECTION
            ))
            .json(&search_body)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();

        latencies.push(start.elapsed());
    }

    latencies.sort();
    let p50 = latencies[NUM_QUERIES / 2];
    let p95 = latencies[(NUM_QUERIES as f64 * 0.95) as usize];
    let p99 = latencies[(NUM_QUERIES as f64 * 0.99) as usize];

    println!("Search latency results:");
    println!("  P50: {:?}", p50);
    println!("  P95: {:?}", p95);
    println!("  P99: {:?}", p99);

    // 4. Filtered search test
    println!("\nRunning filtered search queries...");
    let mut filtered_latencies = Vec::with_capacity(NUM_QUERIES);

    for _ in 0..NUM_QUERIES {
        let query_vector: Vec<f32> = (0..VECTOR_SIZE).map(|_| rng.gen()).collect();

        let start = Instant::now();
        let search_body = serde_json::json!({
            "vector": query_vector,
            "limit": 10,
            "filter": {
                "must": [
                    {"key": "memory_type", "match": {"value": "episodic"}}
                ]
            },
            "with_payload": true
        });

        client
            .post(format!(
                "{}/collections/{}/points/search",
                base_url, COLLECTION
            ))
            .json(&search_body)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();

        filtered_latencies.push(start.elapsed());
    }

    filtered_latencies.sort();
    let fp50 = filtered_latencies[NUM_QUERIES / 2];
    let fp95 = filtered_latencies[(NUM_QUERIES as f64 * 0.95) as usize];
    let fp99 = filtered_latencies[(NUM_QUERIES as f64 * 0.99) as usize];

    println!("Filtered search latency results:");
    println!("  P50: {:?}", fp50);
    println!("  P95: {:?}", fp95);
    println!("  P99: {:?}", fp99);

    // 5. Memory check
    println!("\n=== Summary ===");
    println!("Vectors inserted: {}", NUM_VECTORS);
    println!("Insert time: {:.2}s", insert_time.as_secs_f64());
    println!(
        "Insert throughput: {:.0} vectors/sec",
        NUM_VECTORS as f64 / insert_time.as_secs_f64()
    );
    println!("\nUnfiltered search:");
    println!("  P50={:?}, P95={:?}, P99={:?}", p50, p95, p99);
    println!("Filtered search:");
    println!("  P50={:?}, P95={:?}, P99={:?}", fp50, fp95, fp99);

    // Pass/Fail
    println!("\n=== Pass/Fail Criteria ===");
    println!(
        "P95 < 5ms: {} ({:?})",
        if p95.as_millis() < 5 { "PASS" } else { "FAIL" },
        p95
    );
    println!(
        "Filtered P95 < 10ms: {} ({:?})",
        if fp95.as_millis() < 10 {
            "PASS"
        } else {
            "FAIL"
        },
        fp95
    );
}
