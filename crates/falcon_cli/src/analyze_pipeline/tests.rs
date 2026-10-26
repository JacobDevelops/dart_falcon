use super::{WriteSnapshot, fix_report};
use std::fs;
use std::time::{Duration, SystemTime};

#[test]
fn changed_length_skips_write_and_reports_without_leaving_a_tempfile() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.dart");
    fs::write(&path, "original").unwrap();
    let snapshot = WriteSnapshot::capture(&path, "original").unwrap();
    fs::write(&path, "user saved a longer source").unwrap();
    let error = snapshot.write(&path, "fixed").unwrap_err();
    let report = fix_report(&path, error);
    assert_eq!(report.rule, "autofix");
    assert!(report.message.contains("mtime or length"));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "user saved a longer source"
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn changed_mtime_with_same_length_skips_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.dart");
    fs::write(&path, "original").unwrap();
    let snapshot = WriteSnapshot::capture(&path, "original").unwrap();
    fs::write(&path, "new text").unwrap();
    let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    assert_ne!(snapshot.modified, modified);
    let error = snapshot.write(&path, "fixed").unwrap_err();
    assert!(error.contains("mtime or length"));
    assert_eq!(fs::read_to_string(path).unwrap(), "new text");
}

#[test]
fn changes_between_source_read_and_snapshot_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.dart");
    fs::write(&path, "user edit").unwrap();
    let error = WriteSnapshot::capture(&path, "old source").err().unwrap();
    assert!(error.contains("changed since it was read"));
    assert_eq!(fs::read_to_string(path).unwrap(), "user edit");
}

#[cfg(unix)]
#[test]
fn replacing_source_with_symlink_after_snapshot_skips_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.dart");
    let target = dir.path().join("target.dart");
    fs::write(&path, "original").unwrap();
    fs::write(&target, "external").unwrap();
    let snapshot = WriteSnapshot::capture(&path, "original").unwrap();
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    let error = snapshot.write(&path, "fixed").unwrap_err();
    assert!(error.contains("symlink"));
    assert!(fs::symlink_metadata(path).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(target).unwrap(), "external");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}
