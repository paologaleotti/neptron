use std::fs;

use super::*;
use crate::test_dir::TestDir;

fn put(dir: &TestDir, relative: &str, text: &str) -> PathBuf {
    let path = dir.file(relative);
    fs::write(&path, text).unwrap();
    path
}

fn missing(file: &str, list: &str) -> Result<DiscPlan, DiscProblem> {
    Err(DiscProblem::Missing {
        file: file.to_owned(),
        list: list.to_owned(),
    })
}

#[test]
fn a_single_chd_is_one_disc() {
    let dir = TestDir::new("plan-chd");
    let chd = dir.file("Game.chd");
    let plan = plan_discs(&chd, DiscFormat::Chd).unwrap();
    assert_eq!(
        plan,
        DiscPlan {
            more: Vec::new(),
            numbers: vec![1]
        }
    );
}

#[test]
fn a_cue_needs_every_file_it_names() {
    let dir = TestDir::new("plan-cue");
    let cue = put(
        &dir,
        "Game.cue",
        "FILE \"Game (Track 1).bin\" BINARY\nFILE \"Game (Track 2).bin\" BINARY\n",
    );
    dir.file("Game (Track 1).bin");
    assert_eq!(
        plan_discs(&cue, DiscFormat::Cue),
        missing("Game (Track 2).bin", "Game.cue")
    );
    dir.file("Game (Track 2).bin");
    assert!(plan_discs(&cue, DiscFormat::Cue).is_ok());
}

#[test]
fn a_cue_that_is_too_large_or_has_no_file_line_is_left_to_the_core() {
    let dir = TestDir::new("plan-cue-big");
    let big = put(&dir, "Big.cue", &"REM x\n".repeat(LIST_MAX_BYTES));
    assert!(plan_discs(&big, DiscFormat::Cue).is_ok());
    let empty = put(&dir, "Empty.cue", "REM nothing\n");
    assert!(plan_discs(&empty, DiscFormat::Cue).is_ok());
}

#[test]
fn a_group_takes_its_discs_from_the_folder_in_order() {
    let dir = TestDir::new("plan-group");
    let one = put(&dir, "Saga (Disc 1).cue", "FILE \"s1.bin\" BINARY\n");
    put(&dir, "Saga (Disc 3).cue", "FILE \"s3.bin\" BINARY\n");
    put(&dir, "Saga (Disc 3) .cue", "");
    dir.file("Saga (Disc 2).chd");
    dir.file("s1.bin");
    assert_eq!(
        plan_discs(&one, DiscFormat::Cue),
        missing("s3.bin", "Saga (Disc 3).cue")
    );
    dir.file("s3.bin");
    let plan = plan_discs(&one, DiscFormat::Cue).unwrap();
    assert_eq!(plan.more, [dir.path().join("Saga (Disc 3).cue")]);
    assert_eq!(plan.numbers, [1, 3]);
}

#[test]
fn a_lone_tagged_file_is_one_disc() {
    let dir = TestDir::new("plan-lone");
    let lone = dir.file("Lone (Disc 2).chd");
    let plan = plan_discs(&lone, DiscFormat::Chd).unwrap();
    assert_eq!(plan.numbers, [1]);
    assert!(plan.more.is_empty());
}

#[test]
fn an_m3u_needs_every_disc_and_the_files_of_its_cues() {
    let dir = TestDir::new("plan-m3u");
    let m3u = put(
        &dir,
        "Epic.m3u",
        "#EXTM3U\ndiscs\\Epic 1.cue\n../Epic 2.chd\n",
    );
    put(&dir, "discs/Epic 1.cue", "FILE \"e1.bin\" BINARY\n");
    assert_eq!(
        plan_discs(&m3u, DiscFormat::M3u),
        missing("e1.bin", "Epic 1.cue")
    );
    dir.file("discs/e1.bin");
    let outside = dir.path().parent().unwrap().join("Epic 2.chd");
    assert_eq!(
        plan_discs(&m3u, DiscFormat::M3u),
        missing("Epic 2.chd", "Epic.m3u")
    );
    fs::write(&outside, b"").unwrap();
    let plan = plan_discs(&m3u, DiscFormat::M3u);
    fs::remove_file(&outside).unwrap();
    assert_eq!(
        plan,
        Ok(DiscPlan {
            more: Vec::new(),
            numbers: vec![1, 2]
        })
    );
}

#[test]
fn an_empty_or_unreadable_m3u_names_no_disc() {
    let dir = TestDir::new("plan-m3u-empty");
    let no_disc = |list: &str| {
        Err(DiscProblem::NoDisc {
            list: list.to_owned(),
        })
    };
    let empty = put(&dir, "Empty.m3u", "#EXTM3U\n\n");
    assert_eq!(plan_discs(&empty, DiscFormat::M3u), no_disc("Empty.m3u"));
    let binary = dir.file("Binary.m3u");
    fs::write(&binary, b"\xff\xfe").unwrap();
    assert_eq!(plan_discs(&binary, DiscFormat::M3u), no_disc("Binary.m3u"));
    let gone = dir.path().join("Gone.m3u");
    assert_eq!(plan_discs(&gone, DiscFormat::M3u), no_disc("Gone.m3u"));
}

#[test]
fn a_disc_that_is_not_the_first_of_its_group_is_one_disc_with_its_number() {
    let dir = TestDir::new("plan-not-first");
    dir.file("Saga (Disc 1).chd");
    dir.file("Saga (Disc 2).chd");
    let third = dir.file("Saga (Disc 3).chd");
    put(&dir, "Saga.m3u", "Saga (Disc 1).chd\nSaga (Disc 2).chd\n");
    let plan = plan_discs(&third, DiscFormat::Chd).unwrap();
    assert_eq!(
        plan,
        DiscPlan {
            more: Vec::new(),
            numbers: vec![3]
        },
        "the list shows disc 3 alone, so the core gets disc 3 alone"
    );
    let second = dir.path().join("Saga (Disc 2).chd");
    assert_eq!(plan_discs(&second, DiscFormat::Chd).unwrap().numbers, [2]);
    let first = dir.path().join("Saga (Disc 1).chd");
    let plan = plan_discs(&first, DiscFormat::Chd).unwrap();
    assert_eq!(plan.numbers, [1, 2, 3]);
    assert_eq!(plan.more.len(), 2);
}

#[test]
fn list_and_cue_files_are_found_in_any_case() {
    let dir = TestDir::new("plan-case");
    let m3u = put(&dir, "Epic.m3u", "DISCS/epic 1.CUE\n");
    put(&dir, "discs/Epic 1.cue", "FILE \"TRACK 1.BIN\" BINARY\n");
    dir.file("discs/track 1.bin");
    assert_eq!(
        plan_discs(&m3u, DiscFormat::M3u),
        Ok(DiscPlan {
            more: Vec::new(),
            numbers: vec![1]
        })
    );
    std::fs::remove_file(dir.path().join("discs/track 1.bin")).unwrap();
    assert_eq!(
        plan_discs(&m3u, DiscFormat::M3u),
        missing("TRACK 1.BIN", "Epic 1.cue")
    );
}

#[test]
fn a_list_that_names_a_list_names_no_disc() {
    let dir = TestDir::new("plan-nested");
    let a = put(&dir, "A.m3u", "B.m3u\n");
    let b = put(&dir, "B.m3u", "a.M3U\n");
    for (list, name) in [(&a, "A.m3u"), (&b, "B.m3u")] {
        assert_eq!(
            plan_discs(list, DiscFormat::M3u),
            Err(DiscProblem::NoDisc {
                list: name.to_owned()
            })
        );
    }
}
