use std::time::{Duration, Instant};

use super::*;

fn game(name: &str) -> Game {
    Game::new(name.to_owned(), format!("snes/{name}.sfc"))
}

fn names(list: &SystemGames) -> Vec<&str> {
    list.games().iter().map(Game::name).collect()
}

#[test]
fn names_sort_with_no_regard_to_case() {
    let list = SystemGames::new(
        "snes".into(),
        ["b", "A", "a", "C", "B"].into_iter().map(game).collect(),
    );
    assert_eq!(names(&list), ["A", "a", "B", "b", "C"]);
}

#[test]
fn leading_articles_and_digits_sort_as_written() {
    let list = SystemGames::new(
        "snes".into(),
        ["Zelda", "The Lost Vikings", "10-Yard Fight", "Tetris"]
            .into_iter()
            .map(game)
            .collect(),
    );
    assert_eq!(
        names(&list),
        ["10-Yard Fight", "Tetris", "The Lost Vikings", "Zelda"]
    );
}

#[test]
fn non_ascii_names_sort_by_lower_case_code_point() {
    let list = SystemGames::new(
        "snes".into(),
        ["Zoo", "Écran", "éa", "Ab"].into_iter().map(game).collect(),
    );
    assert_eq!(names(&list), ["Ab", "Zoo", "éa", "Écran"]);
}

#[test]
fn equal_names_keep_both_and_sort_by_path() {
    let list = SystemGames::new(
        "snes".into(),
        vec![
            Game::new("Mario".into(), "snes/usa/Mario.sfc".into()),
            Game::new("Mario".into(), "snes/eur/Mario.sfc".into()),
        ],
    );
    let paths: Vec<&str> = list.games().iter().map(Game::path).collect();
    assert_eq!(paths, ["snes/eur/Mario.sfc", "snes/usa/Mario.sfc"]);
}

#[test]
fn compare_names_is_a_total_order() {
    assert_eq!(compare_names("abc", "ABC"), Ordering::Greater);
    assert_eq!(compare_names("abc", "abc"), Ordering::Equal);
    assert_eq!(compare_names("ab", "abc"), Ordering::Less);
    assert_eq!(compare_names("", "a"), Ordering::Less);
}

#[test]
fn index_drops_empty_systems_and_merges_repeats() {
    let index = Index::from_systems(vec![
        SystemGames::new("nes".into(), vec![]),
        SystemGames::new("snes".into(), vec![game("b")]),
        SystemGames::new("megadrive".into(), vec![game("x")]),
        SystemGames::new("snes".into(), vec![game("a")]),
    ]);
    let ids: Vec<&str> = index.systems().iter().map(SystemGames::system).collect();
    assert_eq!(ids, ["snes", "megadrive"]);
    assert_eq!(names(index.system("snes").unwrap()), ["a", "b"]);
    assert_eq!(index.count("nes"), 0);
    assert_eq!(index.total(), 3);
}

#[test]
fn many_lists_merge_into_one_sorted_list_per_system() {
    let mut lists: Vec<SystemGames> = (0..20_000)
        .map(|i| SystemGames::new(format!("s{i}"), vec![game("x")]))
        .collect();
    lists.extend(
        (0..20_000)
            .rev()
            .map(|i| SystemGames::new("s0".into(), vec![game(&format!("{i:05}"))])),
    );
    let index = Index::from_systems(lists);
    assert_eq!(index.systems().len(), 20_000);
    let first = index.system("s0").expect("the system is there");
    assert_eq!(first.len(), 20_001);
    assert_eq!(first.games()[0].name(), "00000");
    assert_eq!(first.games()[20_000].name(), "x");
    assert_eq!(index.systems()[0].system(), "s0");
}

#[test]
fn page_is_clamped_to_the_list() {
    let index = Index::from_systems(vec![SystemGames::new(
        "snes".into(),
        ["a", "b", "c"].into_iter().map(game).collect(),
    )]);
    assert_eq!(index.page("snes", 1, 10).len(), 2);
    assert_eq!(index.page("snes", 3, 10).len(), 0);
    assert_eq!(index.page("snes", usize::MAX, usize::MAX).len(), 0);
    assert_eq!(index.page("snes", 0, 0).len(), 0);
    assert!(index.page("psx", 0, 10).is_empty());
    assert!(Index::new().systems().is_empty());
}

/// A frame at 60 Hz lasts 16.6 ms. The UI gets a part of it, and a list
/// query must be a small part of that.
const FRAME_BUDGET: Duration = Duration::from_micros(16_667);

/// The limit for one page query. It is 6 % of a frame. The test takes the
/// fastest of many runs, so a busy machine that stops the test for a time
/// does not make it fail; only a query that is slow on every run does.
const PAGE_QUERY_LIMIT: Duration = Duration::from_millis(1);

const GAMES: usize = 10_000;
const ROWS_PER_PAGE: usize = 20;
const RUNS: usize = 200;

fn ten_thousand_games() -> Index {
    let mut seed: u32 = 0x1234_5678;
    let games = (0..GAMES)
        .map(|i| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let name = format!("Game {seed:08x} number {i}");
            Game::new(name.clone(), format!("snes/{name}.sfc"))
        })
        .collect();
    Index::from_systems(vec![
        SystemGames::new("nes".into(), vec![game("only")]),
        SystemGames::new("snes".into(), games),
    ])
}

/// One query as the game list screen makes it: find the system, count its
/// games and read the names of one page of rows.
fn query_page(index: &Index, start: usize) -> (usize, usize) {
    let count = index.count("snes");
    let page = index.page("snes", start, ROWS_PER_PAGE);
    let bytes = page.iter().map(|g| g.name().len()).sum();
    (count, bytes)
}

#[test]
fn ten_thousand_game_query_fits_the_frame_budget() {
    let index = ten_thousand_games();
    let list = index.system("snes").expect("the list has games");
    assert_eq!(list.len(), GAMES);

    // The page is a view into the index: no copy and no allocation per row.
    let all = list.games().as_ptr_range();
    for start in (0..GAMES).step_by(ROWS_PER_PAGE) {
        let page = index.page("snes", start, ROWS_PER_PAGE);
        assert_eq!(page.len(), ROWS_PER_PAGE);
        assert!(all.contains(&page.as_ptr()));
        assert_eq!(page.as_ptr(), list.games()[start..].as_ptr());
    }

    let mut fastest = Duration::MAX;
    let mut checksum = 0;
    for run in 0..RUNS {
        let start = (run * 997) % (GAMES - ROWS_PER_PAGE);
        let t = Instant::now();
        let (count, bytes) = query_page(&index, start);
        fastest = fastest.min(t.elapsed());
        checksum += count + bytes;
    }

    let t = Instant::now();
    for start in (0..GAMES).step_by(ROWS_PER_PAGE) {
        checksum += query_page(&index, start).1;
    }
    let full_pass = t.elapsed();

    eprintln!(
        "10,000-game query: fastest page of {ROWS_PER_PAGE} rows {fastest:?}, \
         all {} pages {full_pass:?}, frame budget {FRAME_BUDGET:?} (checksum {checksum})",
        GAMES / ROWS_PER_PAGE
    );
    assert!(
        fastest <= PAGE_QUERY_LIMIT,
        "the fastest page query took {fastest:?}, the limit is {PAGE_QUERY_LIMIT:?}"
    );
}

#[test]
fn zip_is_told_by_the_extension() {
    assert!(Game::new("a".into(), "nes/a.zip".into()).is_zip());
    assert!(Game::new("a".into(), "nes/a.ZiP".into()).is_zip());
    assert!(!Game::new("a".into(), "nes/a.nes".into()).is_zip());
    assert!(!Game::new("zip".into(), "nes/zip".into()).is_zip());
    assert!(!Game::new("a".into(), "nes/a.zip.nes".into()).is_zip());
}

fn arcade(stem: &str, title: &str, parent: Option<&str>) -> Game {
    Game::new(title.to_owned(), format!("arcade/{stem}.zip")).with_parent(parent.map(str::to_owned))
}

#[test]
fn a_clone_folds_under_a_parent_of_the_same_list() {
    let list = SystemGames::new(
        "arcade".into(),
        vec![
            arcade("b2", "Bee", Some("B1")),
            arcade("b1", "Bee", None),
            arcade("a2", "Ant", Some("a1")),
            arcade("c2", "Cat", Some("c1")),
            arcade("b3", "Bee (rev)", Some("b1")),
        ],
    );
    assert_eq!(names(&list), ["Ant", "Bee", "Cat"]);
    assert_eq!(list.len(), 3);
    let versions: Vec<&str> = list.versions("b1").iter().map(Game::stem).collect();
    assert_eq!(versions, ["b2", "b3"]);
    assert!(list.versions("a2").is_empty());
    assert!(list.versions("").is_empty());
    assert_eq!(list.all().count(), 5);
    assert_eq!(list.games()[1].stem(), "b1");
    let folded: Vec<&str> = list.folded().iter().map(Game::stem).collect();
    assert_eq!(folded, ["b2", "b3"]);
    assert_eq!(list.versions_range("B1"), 0..2);
    assert!(list.versions_range("zz").is_empty());
}

#[test]
fn a_merged_system_folds_across_its_lists() {
    let index = Index::from_systems(vec![
        SystemGames::new("arcade".into(), vec![arcade("b2", "Bee", Some("b1"))]),
        SystemGames::new("arcade".into(), vec![arcade("b1", "Bee", None)]),
    ]);
    let list = index.system("arcade").expect("arcade");
    assert_eq!(list.len(), 1);
    assert_eq!(list.versions("b1").len(), 1);
}

#[test]
fn the_stem_has_no_folder_and_no_extension() {
    assert_eq!(arcade("x", "X", None).stem(), "x");
    assert_eq!(Game::new("n".into(), "snes/a/b.c.sfc".into()).stem(), "b.c");
    assert_eq!(Game::new("n".into(), "snes/noext".into()).stem(), "noext");
}

fn disc(path: &str) -> Game {
    let file = path.rsplit('/').next().unwrap();
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
    Game::new(stem.to_owned(), path.to_owned())
}

fn arranged(games: Vec<Game>) -> SystemGames {
    SystemGames::new("psx".into(), games)
}

fn hidden_paths(list: &SystemGames) -> Vec<&str> {
    list.hidden().iter().map(Game::path).collect()
}

#[test]
fn files_with_a_disc_tag_make_one_entry_in_disc_order() {
    let list = arranged(vec![
        disc("psx/Saga (USA) (Disc 2).chd"),
        disc("psx/Saga (USA) (Disc 1).chd"),
        disc("psx/Saga (USA) (Disc 3 of 3).chd"),
        disc("psx/Other.chd"),
    ]);
    assert_eq!(names(&list), ["Other", "Saga (USA)"]);
    let saga = &list.games()[1];
    assert!(saga.is_group());
    assert_eq!(saga.path(), "psx/Saga (USA) (Disc 1).chd");
    assert_eq!(
        saga.discs(),
        [
            "psx/Saga (USA) (Disc 1).chd",
            "psx/Saga (USA) (Disc 2).chd",
            "psx/Saga (USA) (Disc 3 of 3).chd"
        ]
    );
    assert_eq!(list.hidden().len(), 3);
    assert_eq!(list.all().count(), 4, "the files, not the entry");
}

#[test]
fn a_group_needs_one_folder_one_name_one_format_and_two_numbers() {
    let list = arranged(vec![
        disc("psx/Lone (Disc 1).chd"),
        disc("psx/Mixed (Disc 1).cue"),
        disc("psx/Mixed (Disc 2).chd"),
        disc("psx/a/Split (Disc 1).chd"),
        disc("psx/b/Split (Disc 2).chd"),
        disc("psx/Game (Disc 1).nes"),
        disc("psx/Game (Disc 2).nes"),
    ]);
    assert!(list.hidden().is_empty());
    assert_eq!(list.len(), 7);
    assert!(list.games().iter().all(|g| !g.is_group()));
}

#[test]
fn a_repeated_number_leaves_the_later_path_on_its_own_and_gaps_are_kept() {
    let list = arranged(vec![
        disc("psx/Q (Disc 1).chd"),
        disc("psx/Q (Disc 1 of 3).chd"),
        disc("psx/Q (Disc 3).chd"),
        disc("psx/Q (disc 3).CHD"),
    ]);
    // " of 3" sorts before ")" in bytes.
    assert_eq!(names(&list), ["Q", "Q (Disc 1)", "Q (disc 3)"]);
    let q = list.games().iter().find(|g| g.is_group()).unwrap();
    assert_eq!(q.discs(), ["psx/Q (Disc 1 of 3).chd", "psx/Q (Disc 3).chd"]);
}

#[test]
fn an_m3u_hides_the_files_it_names_in_any_folder() {
    let list = arranged(vec![
        disc("psx/Epic.m3u").with_discs(vec![
            "psx/discs/Epic (Disc 1).cue".into(),
            "psx/discs/Epic (Disc 2).cue".into(),
            "psx/gone.chd".into(),
        ]),
        disc("psx/Twin.m3u").with_discs(vec!["psx/discs/Epic (Disc 2).cue".into()]),
        disc("psx/discs/Epic (Disc 1).cue"),
        disc("psx/discs/Epic (Disc 2).cue"),
        disc("psx/discs/Epic (Disc 3).cue"),
        disc("psx/Self.m3u").with_discs(vec!["psx/Self.m3u".into()]),
    ]);
    assert_eq!(names(&list), ["Epic", "Epic (Disc 3)", "Self", "Twin"]);
    assert_eq!(
        hidden_paths(&list),
        ["psx/discs/Epic (Disc 1).cue", "psx/discs/Epic (Disc 2).cue"]
    );
}

#[test]
fn a_group_is_made_again_from_its_files_when_lists_merge() {
    let first = arranged(vec![disc("psx/S (Disc 1).chd")]);
    let second = arranged(vec![disc("psx/S (Disc 2).chd")]);
    let index = Index::from_systems(vec![first, second]);
    let list = index.system("psx").unwrap();
    assert_eq!(names(list), ["S"]);
    let again = Index::from_systems(vec![list.clone(), arranged(vec![disc("psx/T.chd")])]);
    let list = again.system("psx").unwrap();
    assert_eq!(names(list), ["S", "T"]);
    assert_eq!(list.games()[0].discs().len(), 2);
}

#[test]
fn an_m3u_line_names_a_file_in_any_case() {
    let list = arranged(vec![
        disc("psx/Epic.m3u").with_discs(vec!["psx/DISCS/epic (disc 1).CUE".into()]),
        disc("psx/discs/Epic (Disc 1).cue"),
        disc("psx/discs/EPIC (DISC 1).cue"),
    ]);
    assert_eq!(names(&list), ["Epic"]);
    assert_eq!(
        list.hidden().len(),
        2,
        "on ext4 both files of one name in two cases"
    );
}

#[test]
fn a_line_that_names_a_list_hides_nothing() {
    let list = arranged(vec![
        disc("psx/A.m3u").with_discs(vec!["psx/B.m3u".into()]),
        disc("psx/B.m3u").with_discs(vec!["psx/a.M3U".into()]),
        disc("psx/Self.m3u").with_discs(vec!["psx/Self.m3u".into()]),
    ]);
    assert_eq!(names(&list), ["A", "B", "Self"]);
    assert!(list.hidden().is_empty());
}
