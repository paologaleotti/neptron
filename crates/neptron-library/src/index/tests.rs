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
