use super::*;

#[test]
fn a_disc_tag_gives_the_number_and_the_group_name() {
    let tag = disc_tag("Final Fantasy VII (USA) (Disc 1)").unwrap();
    assert_eq!(tag.number, 1);
    assert_eq!(tag.group, "Final Fantasy VII (USA)");
    let tag = disc_tag("Game (Disc 2) (Europe)").unwrap();
    assert_eq!((tag.number, tag.group.as_str()), (2, "Game (Europe)"));
    let tag = disc_tag("Game (disc 3 of 4)").unwrap();
    assert_eq!((tag.number, tag.group.as_str()), (3, "Game"));
    let tag = disc_tag("(Disc 1)Game").unwrap();
    assert_eq!(tag.group, "Game");
}

#[test]
fn only_a_real_tag_counts() {
    for stem in [
        "Game",
        "Game (Disk 1)",
        "Game (Disc)",
        "Game (Disc 0)",
        "Game (Disc 100)",
        "Game (Disc 01x)",
        "Game (Disc -1)",
        "Game (Disc 1 of)",
        "Game (Disc 1 of 0)",
        "Game (Disc 1 from 2)",
        "Game (Disc 1",
        "Game (Discs 1)",
        "Game (Disc  1)",
    ] {
        assert_eq!(disc_tag(stem), None, "{stem}");
    }
}

#[test]
fn the_first_tag_counts() {
    let tag = disc_tag("Game (Disc 2) (Disc 1)").unwrap();
    assert_eq!((tag.number, tag.group.as_str()), (2, "Game (Disc 1)"));
    assert_eq!(disc_tag("Game (Disc 99)").unwrap().number, 99);
    assert_eq!(disc_tag("Game (Disc 07)").unwrap().number, 7);
}

#[test]
fn the_save_name_drops_the_tag() {
    assert_eq!(save_name("Game (USA) (Disc 2)"), "Game (USA)");
    assert_eq!(save_name("Game (USA)"), "Game (USA)");
}

#[test]
fn the_region_comes_from_the_first_group_with_a_region_word() {
    assert_eq!(region_of("Game (USA, Europe)"), Some(Region::NtscU));
    assert_eq!(region_of("Game (Japan) (En)"), Some(Region::NtscJ));
    assert_eq!(region_of("Game (Disc 1) (europe)"), Some(Region::Pal));
    assert_eq!(region_of("Game (En,Fr) (Germany)"), Some(Region::Pal));
    assert_eq!(region_of("Game (E)"), Some(Region::Pal));
    assert_eq!(region_of("Game (Rev 1)"), None);
    assert_eq!(region_of("Game"), None);
    assert_eq!(region_of("Game (World)"), None);
}

#[test]
fn region_tokens_and_words() {
    assert_eq!(Region::of_token("pal"), Some(Region::Pal));
    assert_eq!(Region::of_token("PAL"), None);
    assert_eq!(Region::NtscJ.word(), "Japan");
    assert_eq!(Region::NtscU.word(), "USA");
    assert_eq!(Region::Pal.word(), "Europe");
}

#[test]
fn disc_formats_ignore_case() {
    assert_eq!(DiscFormat::of_extension("CHD"), Some(DiscFormat::Chd));
    assert_eq!(DiscFormat::of_extension("cue"), Some(DiscFormat::Cue));
    assert_eq!(DiscFormat::of_extension("m3u"), Some(DiscFormat::M3u));
    assert_eq!(DiscFormat::of_extension("bin"), None);
}

#[test]
fn m3u_lines_skip_comments_and_blanks() {
    let text = "#EXTM3U\r\n Game (Disc 1).chd \r\n\r\n#COVER x.png\nGame (Disc 2).chd\n";
    assert_eq!(m3u_lines(text), ["Game (Disc 1).chd", "Game (Disc 2).chd"]);
    let long: String = (0..100).map(|i| format!("d{i}.chd\n")).collect();
    let lines = m3u_lines(&long);
    assert_eq!(lines.len(), M3U_MAX_DISCS);
    assert_eq!(lines[63], "d63.chd");
}

#[test]
fn m3u_paths_stay_in_the_folder() {
    assert_eq!(
        m3u_disc_path("psx/Game.m3u", "discs\\Game (Disc 1).chd").as_deref(),
        Some("psx/discs/Game (Disc 1).chd")
    );
    assert_eq!(
        m3u_disc_path("psx/a/Game.m3u", "Game.cue").as_deref(),
        Some("psx/a/Game.cue")
    );
    for line in [
        "../Game.cue",
        "/abs/Game.cue",
        "\\abs",
        "C:\\Game.cue",
        "a//b.cue",
        "./b.cue",
    ] {
        assert_eq!(m3u_disc_path("psx/Game.m3u", line), None, "{line}");
    }
}

#[test]
fn cue_files_read_quoted_and_plain_names() {
    let text = "REM x\nFILE \"Game (Track 1).bin\" BINARY\n  TRACK 01 MODE2/2352\n\tfile track2.bin BINARY\nFILEX x\nFILE \"\" BINARY\nFILE\n";
    assert_eq!(cue_files(text), ["Game (Track 1).bin", "track2.bin"]);
}
