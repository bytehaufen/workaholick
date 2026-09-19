use std::{fs, process::Command};

#[test]
fn day_without_records_exits_with_error() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let store_path = dir.path().join("records.jsonl");

    let store_path = toml::Value::String(store_path.to_str().unwrap().to_owned());

    fs::write(&config_path, format!("store_path = {store_path}\n")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_workaholick"))
        .arg("--config")
        .arg(&config_path)
        .arg("day")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(String::from_utf8(output.stderr).unwrap().trim(), "no records exist");
}

#[test]
fn day_without_records_for_date_exits_with_error() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let store_path = dir.path().join("records.jsonl");

    let store_path = toml::Value::String(store_path.to_str().unwrap().to_owned());

    fs::write(&config_path, format!("store_path = {store_path}\n")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_workaholick"))
        .arg("--config")
        .arg(&config_path)
        .arg("work")
        .arg("start")
        .arg("office")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "✅ Record with ID 0 added.");

    let output = Command::new(env!("CARGO_BIN_EXE_workaholick"))
        .arg("--config")
        .arg(&config_path)
        .arg("day")
        .arg("0001-01-01")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap().trim(),
        "no records for date 0001-01-01 exist"
    );
}
