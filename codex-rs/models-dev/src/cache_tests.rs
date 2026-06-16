use crate::ModelsDevCache;
use crate::ModelsDevProvider;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::sync::Barrier;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;
use tempfile::TempDir;

fn sample_provider(id: &str) -> ModelsDevProvider {
    ModelsDevProvider {
        id: id.to_string(),
        name: id.to_string(),
        env: vec!["TEST_API_KEY".to_string()],
        api: Some(format!("https://example.test/{id}/v1")),
        npm: None,
        models: HashMap::new(),
    }
}

#[test]
fn cache_hit_when_fresh() {
    let temp = TempDir::new().expect("temp dir");
    let cache_path = temp.path().join("models-dev.json");
    let providers = HashMap::from([("anthropic".to_string(), sample_provider("anthropic"))]);
    fs::write(
        &cache_path,
        serde_json::to_string(&providers).expect("json"),
    )
    .expect("write");

    let cache = ModelsDevCache::with_paths(cache_path, "http://127.0.0.1:1".to_string());
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let loaded = rt
        .block_on(cache.get(/*force_refresh*/ false))
        .expect("load");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded["anthropic"].id, "anthropic");
}

#[test]
fn cache_miss_when_stale() {
    let temp = TempDir::new().expect("temp dir");
    let cache_path = temp.path().join("models-dev.json");
    fs::write(&cache_path, "{}").expect("write");
    let past = SystemTime::now() - Duration::from_secs(600);
    filetime::set_file_mtime(&cache_path, filetime::FileTime::from_system_time(past));

    let cache = ModelsDevCache::with_paths(cache_path, "http://127.0.0.1:1".to_string());
    assert!(!cache.is_fresh_on_disk().expect("fresh check"));
}

#[test]
fn corrupt_cache_is_removed() {
    let temp = TempDir::new().expect("temp dir");
    let cache_path = temp.path().join("models-dev.json");
    fs::write(&cache_path, "{not-json").expect("write");

    let cache = ModelsDevCache::with_paths(cache_path.clone(), "http://127.0.0.1:1".to_string());
    let loaded = cache.load_from_disk().expect("load");
    assert!(loaded.is_none());
    assert!(!cache_path.exists());
}

#[test]
fn lock_contention_two_writers() {
    let temp = TempDir::new().expect("temp dir");
    let cache_path = temp.path().join("models-dev.json");
    let cache = Arc::new(ModelsDevCache::with_paths(
        cache_path,
        "http://127.0.0.1:1".to_string(),
    ));
    let barrier = Arc::new(Barrier::new(2));
    let cache_a = Arc::clone(&cache);
    let cache_b = Arc::clone(&cache);
    let barrier_a = Arc::clone(&barrier);
    let barrier_b = Arc::clone(&barrier);

    let writer_a = thread::spawn(move || {
        barrier_a.wait();
        cache_a
            .write_with_lock_for_test(
                r#"{"a":{"id":"a","name":"a","env":[],"models":{}}}"#,
                /*force*/ true,
            )
            .expect("write a");
    });
    let writer_b = thread::spawn(move || {
        barrier_b.wait();
        cache_b
            .write_with_lock_for_test(
                r#"{"b":{"id":"b","name":"b","env":[],"models":{}}}"#,
                /*force*/ true,
            )
            .expect("write b");
    });

    writer_a.join().expect("writer a");
    writer_b.join().expect("writer b");

    let contents = fs::read_to_string(cache.cache_path()).expect("read cache");
    assert!(contents.contains("\"a\"") || contents.contains("\"b\""));
}
