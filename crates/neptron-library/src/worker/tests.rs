use super::*;
use crate::test_dir::TestDir;

fn catalog() -> Catalog {
    Catalog::builtin().expect("the built-in definitions are valid")
}

fn fixture() -> TestDir {
    let dir = TestDir::new("worker");
    for file in ["nes/a.nes", "nes/b.nes", "snes/c.sfc", "megadrive/d.md"] {
        dir.file(file);
    }
    dir
}

#[test]
fn worker_returns_the_scan_result() {
    let dir = fixture();
    let worker = ScanWorker::spawn(dir.path().to_path_buf(), catalog()).expect("the thread starts");
    let outcome = worker.join().expect("the scan completes");
    assert_eq!(outcome.index.total(), 4);
}

#[test]
fn progress_reaches_the_final_counts() {
    let dir = fixture();
    let worker = ScanWorker::spawn(dir.path().to_path_buf(), catalog()).expect("the thread starts");
    let done = ScanProgress {
        folders: 4,
        games: 4,
        kept: 0,
    };
    // The counts only grow, so the loop ends when the scan reports its last folder.
    while worker.progress() != done {
        std::thread::yield_now();
    }
    assert!(worker.join().is_ok());
}

#[test]
fn worker_reports_a_missing_root() {
    let dir = TestDir::new("worker-missing");
    let worker = ScanWorker::spawn(dir.path().join("roms"), catalog()).expect("the thread starts");
    assert!(matches!(worker.join(), Err(ScanError::Root { .. })));
}

#[test]
fn drop_cancels_and_joins() {
    let dir = fixture();
    let worker = ScanWorker::spawn(dir.path().to_path_buf(), catalog()).expect("the thread starts");
    drop(worker);
}

#[test]
fn cancelled_worker_ends() {
    let dir = fixture();
    let worker = ScanWorker::spawn(dir.path().to_path_buf(), catalog()).expect("the thread starts");
    worker.cancel();
    // The scan can end before it sees the flag; either result is correct.
    match worker.join() {
        Ok(_) | Err(ScanError::Cancelled) => {}
        Err(e) => panic!("unexpected error {e}"),
    }
}

#[test]
fn a_refresh_worker_gives_the_same_games() {
    let dir = fixture();
    let first = ScanWorker::spawn(dir.path().to_path_buf(), catalog())
        .expect("the thread starts")
        .join()
        .expect("the scan completes");
    // Far enough after the scan that every folder time is trusted.
    let later = SystemTime::now() + std::time::Duration::from_secs(3600);
    let worker = ScanWorker::spawn_refresh(
        dir.path().to_path_buf(),
        catalog(),
        first.index.clone(),
        later,
    )
    .expect("the thread starts");
    let outcome = worker.join().expect("the refresh completes");
    assert_eq!(outcome.index.systems(), first.index.systems());
    assert!(outcome.index.folders().iter().all(|f| f.mtime.is_some()));
}

#[test]
fn drop_of_a_refresh_worker_cancels_and_joins() {
    let dir = fixture();
    let worker = ScanWorker::spawn_refresh(
        dir.path().to_path_buf(),
        catalog(),
        Index::new(),
        SystemTime::now(),
    )
    .expect("the thread starts");
    drop(worker);
}
