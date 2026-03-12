use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tempfile::tempdir;

use osiris::db;

#[test]
fn test_database_operations() {
    let dir = tempdir().unwrap();
    let db_path: PathBuf = dir.path().join("osiris_test.db");

    db::create_db(&db_path).unwrap();

    let ts1 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let ts2 = ts1 + 10;

    let emb1 = vec![0u8; 10];
    let emb2 = vec![1u8; 10];

    // Insert
    let id1 = db::insert_entry(&db_path, "Text 1", ts1, &emb1, "App1", "Title1").unwrap();
    assert!(id1.is_some());

    let id2 = db::insert_entry(&db_path, "Text 2", ts2, &emb2, "App2", "Title2").unwrap();
    assert!(id2.is_some());

    // Duplicate timestamp
    let id_dup = db::insert_entry(&db_path, "Text 3", ts1, &emb1, "App3", "Title3").unwrap();
    assert!(id_dup.is_none());

    // Get all
    let entries = db::get_all_entries(&db_path).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].timestamp, ts2); // ordered DESC
    assert_eq!(entries[0].text, "Text 2");
    assert_eq!(entries[1].timestamp, ts1);

    // Get timestamps
    let timestamps = db::get_timestamps(&db_path).unwrap();
    assert_eq!(timestamps, vec![ts2, ts1]);
}
