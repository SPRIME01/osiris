use std::path::PathBuf;

use osiris::config;

#[test]
fn test_get_appdata_folder() {
    let cfg = config::Config {
        storage_path: Some(PathBuf::from("/tmp/osiris_test_storage")),
        primary_monitor_only: false,
    };
    assert_eq!(cfg.appdata_folder(), PathBuf::from("/tmp/osiris_test_storage"));
}
