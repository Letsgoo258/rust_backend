use std::sync::Arc;
use tokio::task::JoinHandle;
use uuid::Uuid;

#[tokio::test]
async fn test_key_lock_manager_serialized_execution() {
    let lock_mgr = eravaya::utils::concurrency::KeyLockManager::new();
    let school_id = Uuid::new_v4();
    let shared_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut handles: Vec<JoinHandle<()>> = Vec::new();

    // Spawn 50 concurrent tasks locking on the SAME key
    for _ in 0..50 {
        let lm = lock_mgr.clone();
        let sc = shared_counter.clone();
        handles.push(tokio::spawn(async move {
            let key = (school_id, "user:email:teacher@school.com".to_string());
            let _guard = lm.acquire_lock(&key).await;

            // Critical section
            let cur = sc.load(std::sync::atomic::Ordering::SeqCst);
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            sc.store(cur + 1, std::sync::atomic::Ordering::SeqCst);
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(shared_counter.load(std::sync::atomic::Ordering::SeqCst), 50);
}

#[tokio::test]
async fn test_dashmap_job_store_concurrent_access() {
    let job_store = eravaya::utils::jobs::create_job_store();
    let job_id = Uuid::new_v4();
    let school_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let job = eravaya::utils::jobs::BulkUploadJob {
        id: job_id,
        school_id,
        user_id,
        job_type: "STAFF_BULK_UPLOAD".to_string(),
        status: eravaya::utils::jobs::JobStatus::PROCESSING,
        total_rows: 100,
        processed_rows: 0,
        success_count: 0,
        failed_count: 0,
        failed_csv: None,
        error: None,
        created_at: 0,
        updated_at: 0,
    };

    job_store.insert(job_id, job);

    let mut handles: Vec<JoinHandle<()>> = Vec::new();

    // 20 concurrent threads incrementally reporting processed rows
    for _ in 0..20 {
        let store = job_store.clone();
        handles.push(tokio::spawn(async move {
            if let Some(mut entry) = store.get_mut(&job_id) {
                entry.processed_rows += 1;
                entry.success_count += 1;
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let final_job = job_store.get(&job_id).unwrap();
    assert_eq!(final_job.processed_rows, 20);
    assert_eq!(final_job.success_count, 20);
}
