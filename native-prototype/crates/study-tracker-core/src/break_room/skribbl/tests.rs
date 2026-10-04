use super::fill::flood_fill;
use super::zurich::{is_summer_time, zurich_date, zurich_day_start};
use super::*;
use crate::dashboard::civil::CivilDate;
use crate::social::time::SocialTimestamp;
use crate::timer::WallTimestamp;

fn at(iso: &str) -> WallTimestamp {
    SocialTimestamp::parse(iso).unwrap().wall()
}

fn did(s: &str) -> DrawingId {
    DrawingId::parse(s).unwrap()
}

fn theme(submitted: bool) -> ThemeInfo {
    ThemeInfo {
        date: "2026-10-04".into(),
        theme: "Lighthouse in the fog".into(),
        submitted,
        drawing_id: submitted.then(|| did("mine")),
        image_url: submitted.then(|| "https://w/skribbl/drawing/x".into()),
    }
}

fn drawing(id: &str, is_self: bool, score: i64, my_vote: i8) -> Drawing {
    Drawing {
        id: did(id),
        user_id: UserId::parse(&format!("u-{id}")).unwrap(),
        display_name: format!("Artist {id}"),
        vote_score: score,
        vote_count: u64::from(my_vote != 0),
        my_vote,
        is_self,
        image_url: format!("https://w/skribbl/drawing/{id}"),
    }
}

// ------------------------------------------------------------------------------------- Zurich

#[test]
fn zurich_dates_follow_cet_and_cest() {
    // winter: UTC+1
    assert_eq!(
        zurich_date(at("2026-01-15T22:59:59Z")).to_iso(),
        "2026-01-15"
    );
    assert_eq!(
        zurich_date(at("2026-01-15T23:00:00Z")).to_iso(),
        "2026-01-16",
        "UTC date differs from Zurich date"
    );
    // summer: UTC+2
    assert_eq!(
        zurich_date(at("2026-07-01T21:59:59Z")).to_iso(),
        "2026-07-01"
    );
    assert_eq!(
        zurich_date(at("2026-07-01T22:00:00Z")).to_iso(),
        "2026-07-02"
    );
    assert!(!is_summer_time(at("2026-01-15T12:00:00Z")));
    assert!(is_summer_time(at("2026-07-01T12:00:00Z")));
}

#[test]
fn zurich_dst_transitions_are_exact() {
    // 2026: summer time from Sunday 29 March 01:00 UTC to Sunday 25 October 01:00 UTC
    assert!(!is_summer_time(at("2026-03-29T00:59:59Z")));
    assert!(is_summer_time(at("2026-03-29T01:00:00Z")));
    assert!(is_summer_time(at("2026-10-25T00:59:59Z")));
    assert!(!is_summer_time(at("2026-10-25T01:00:00Z")));
    // 2027: 28 March / 31 October
    assert!(is_summer_time(at("2027-03-28T01:00:00Z")));
    assert!(!is_summer_time(at("2027-10-31T01:00:00Z")));
    // the Zurich midnight on either side of a transition day
    assert_eq!(
        zurich_day_start(CivilDate::from_ymd(2026, 3, 29).unwrap()),
        at("2026-03-28T23:00:00Z"),
        "CET midnight"
    );
    assert_eq!(
        zurich_day_start(CivilDate::from_ymd(2026, 3, 30).unwrap()),
        at("2026-03-29T22:00:00Z"),
        "CEST midnight"
    );
    assert_eq!(
        zurich_day_start(CivilDate::from_ymd(2026, 10, 25).unwrap()),
        at("2026-10-24T22:00:00Z")
    );
    assert_eq!(
        zurich_day_start(CivilDate::from_ymd(2026, 10, 26).unwrap()),
        at("2026-10-25T23:00:00Z")
    );
    // a whole Zurich day maps back to itself, hour by hour, across both transitions
    for day in [
        CivilDate::from_ymd(2026, 3, 29).unwrap(),
        CivilDate::from_ymd(2026, 10, 25).unwrap(),
    ] {
        let start = zurich_day_start(day).unix_millis;
        let end = zurich_day_start(day.add_days(1)).unix_millis;
        assert!(
            end - start == 23 * 3_600_000 || end - start == 25 * 3_600_000,
            "transition days are 23/25 h"
        );
        let mut t = start;
        while t < end {
            assert_eq!(zurich_date(WallTimestamp::from_unix_millis(t)), day);
            t += 15 * 60_000;
        }
        assert_eq!(
            zurich_date(WallTimestamp::from_unix_millis(end)),
            day.add_days(1)
        );
    }
}

#[test]
fn zurich_date_is_independent_of_the_machine_timezone() {
    // the function takes an instant only: a New York or Tokyo machine gets the same answer
    let instant = at("2026-10-04T22:30:00Z"); // 00:30 in Zurich (CEST), 18:30 in New York, 07:30 in Tokyo
    assert_eq!(zurich_date(instant).to_iso(), "2026-10-05");
}

// ------------------------------------------------------------------------------------- fill

fn canvas(w: usize, h: usize, rgba: [u8; 4]) -> Vec<u8> {
    rgba.iter().copied().cycle().take(w * h * 4).collect()
}

fn px(data: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
    let i = (y * w + x) * 4;
    [data[i], data[i + 1], data[i + 2], data[i + 3]]
}

#[test]
fn fill_covers_an_empty_canvas_and_ignores_same_colour() {
    let white = [255, 255, 255, 255];
    let red = [229, 57, 53, 255];
    let mut c = canvas(CANVAS_W, CANVAS_H, white);
    let out = flood_fill(&mut c, CANVAS_W, CANVAS_H, 0, 0, red, FILL_TOLERANCE);
    assert_eq!(out.filled, CANVAS_W * CANVAS_H);
    assert!(!out.capped);
    assert_eq!(px(&c, CANVAS_W, CANVAS_W - 1, CANVAS_H - 1), red);
    // filling with (nearly) the same colour does nothing
    let out = flood_fill(
        &mut c,
        CANVAS_W,
        CANVAS_H,
        450,
        300,
        [230, 60, 50, 255],
        FILL_TOLERANCE,
    );
    assert_eq!(out.filled, 0);
    // repeated full-canvas fills stay bounded and exact
    for i in 0..10u8 {
        let colour = if i % 2 == 0 { white } else { red };
        let out = flood_fill(&mut c, CANVAS_W, CANVAS_H, 899, 599, colour, FILL_TOLERANCE);
        assert_eq!(out.filled, CANVAS_W * CANVAS_H);
    }
}

#[test]
fn fill_stops_at_an_enclosing_border_and_respects_tolerance() {
    let (w, h) = (40, 30);
    let white = [255, 255, 255, 255];
    let black = [0, 0, 0, 255];
    let mut c = canvas(w, h, white);
    // a 10x10 black square outline at (10,10)..(19,19)
    for i in 10..20 {
        for (x, y) in [(i, 10), (i, 19), (10, i), (19, i)] {
            let j = (y * w + x) * 4;
            c[j..j + 4].copy_from_slice(&black);
        }
    }
    // a near-white pixel inside (within tolerance) is filled too
    let j = (15 * w + 15) * 4;
    c[j..j + 4].copy_from_slice(&[230, 230, 230, 255]);
    let blue = [30, 136, 229, 255];
    let out = flood_fill(&mut c, w, h, 12, 12, blue, FILL_TOLERANCE);
    assert_eq!(out.filled, 8 * 8, "only the inside");
    assert_eq!(px(&c, w, 15, 15), blue);
    assert_eq!(px(&c, w, 5, 5), white, "outside untouched");
    assert_eq!(px(&c, w, 10, 10), black, "border untouched");
    // a corner start
    let out = flood_fill(&mut c, w, h, 0, 0, blue, FILL_TOLERANCE);
    assert_eq!(
        out.filled,
        w * h - 8 * 8 - 36,
        "everything outside the square"
    );
}

#[test]
fn fill_rejects_bad_input_without_panicking() {
    let mut c = canvas(4, 4, [0, 0, 0, 255]);
    assert_eq!(flood_fill(&mut c, 4, 4, 4, 0, [1, 2, 3, 255], 40).filled, 0);
    assert_eq!(flood_fill(&mut c, 4, 4, 0, 9, [1, 2, 3, 255], 40).filled, 0);
    let mut short = vec![0u8; 10];
    assert_eq!(flood_fill(&mut short, 4, 4, 0, 0, [255; 4], 40).filled, 0);
    let mut empty: Vec<u8> = Vec::new();
    assert_eq!(flood_fill(&mut empty, 0, 0, 0, 0, [255; 4], 40).filled, 0);
}

#[test]
fn fill_handles_a_maze_without_recursion() {
    // a serpentine of 1-px walls: worst case for span fills, still O(pixels)
    let (w, h) = (CANVAS_W, CANVAS_H);
    let mut c = canvas(w, h, [255; 4]);
    for x in (2..w).step_by(4) {
        let gap = if (x / 4) % 2 == 0 { 0 } else { h - 1 };
        for y in 0..h {
            if y != gap {
                let j = (y * w + x) * 4;
                c[j..j + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
    let out = flood_fill(&mut c, w, h, 0, 0, [67, 160, 71, 255], FILL_TOLERANCE);
    assert!(!out.capped);
    assert!(out.filled > w * h / 2);
}

// ----------------------------------------------------------------------------- state machine

#[test]
fn without_an_identity_nothing_is_requested() {
    let (mut s, effects) = Session::open(false);
    assert!(effects.is_empty());
    assert_eq!(s.phase, Phase::Intro);
    assert!(s.retry().is_empty());
    s.start_drawing(0);
    assert_eq!(s.phase, Phase::Intro, "no drawing without the server");
    assert!(s.load_gallery(0).is_empty());
}

#[test]
fn fresh_day_flow_draw_submit_gallery() {
    let (mut s, effects) = Session::open(true);
    assert_eq!(effects, [Effect::LoadTheme]);
    assert_eq!(s.phase, Phase::Loading);
    let effects = s.theme_loaded(theme(false));
    assert_eq!(
        effects,
        [Effect::LoadWinner],
        "no gallery before submitting"
    );
    assert_eq!(
        (s.phase, s.theme.as_str()),
        (Phase::Intro, "Lighthouse in the fog")
    );
    s.start_drawing(1_000);
    assert_eq!(s.phase, Phase::Drawing);
    assert_eq!(s.time_display(1_000), "3:00");
    assert_eq!(s.time_display(1_999), "3:00");
    assert_eq!(s.time_display(2_000), "2:59");
    assert!(!s.time_low(1_000 + 149_000));
    assert!(s.time_low(1_000 + 150_000), "0:30 is low");
    let effects = s.submit();
    assert_eq!(
        effects,
        [Effect::Submit {
            date: "2026-10-04".into()
        }]
    );
    assert!(s.submit().is_empty(), "double click submits once");
    assert_eq!(s.phase, Phase::Submitting);
    let effects = s.submit_succeeded("https://w/skribbl/drawing/me".into());
    assert_eq!(effects, [Effect::LoadGallery { offset: 0 }]);
    assert!(s.submitted && s.phase == Phase::Submitted);
    s.gallery_loaded(
        0,
        GalleryPage {
            drawings: vec![drawing("a", true, 0, 0), drawing("b", false, 2, 0)],
            next_offset: None,
            has_more: false,
        },
    );
    assert_eq!(s.gallery_count_label(), "2 drawings");
    assert!(!s.gallery_has_more);
}

#[test]
fn already_submitted_today_shows_gallery_at_once() {
    let (mut s, _) = Session::open(true);
    let effects = s.theme_loaded(theme(true));
    assert_eq!(
        effects,
        [Effect::LoadGallery { offset: 0 }, Effect::LoadWinner]
    );
    s.start_drawing(0);
    assert_eq!(s.phase, Phase::Intro, "one drawing per day");
}

#[test]
fn the_deadline_auto_submits_exactly_once_whatever_the_frame_timing() {
    let (mut s, _) = Session::open(true);
    s.theme_loaded(theme(false));
    s.start_drawing(10_000);
    assert!(s.tick(10_000 + 179_999).is_empty());
    // a single late frame (hidden window, sleep): the deadline is still honoured once
    let effects = s.tick(10_000 + 900_000);
    assert_eq!(
        effects,
        [Effect::Submit {
            date: "2026-10-04".into()
        }]
    );
    assert!(s.tick(10_000 + 900_001).is_empty());
    // the upload fails offline: back to drawing at 0:00, no automatic retry loop
    s.submit_failed("Could not reach the Daily Skribbl server.".into());
    assert_eq!(s.phase, Phase::Drawing);
    for t in 0..100 {
        assert!(
            s.tick(10_000 + 900_002 + t * 16).is_empty(),
            "no busy retry"
        );
    }
    assert_eq!(s.time_display(10_000 + 1_000_000), "0:00");
    // a manual submit still works
    assert_eq!(s.submit().len(), 1);
}

#[test]
fn a_409_or_any_failure_keeps_the_drawing_phase_and_message() {
    let (mut s, _) = Session::open(true);
    s.theme_loaded(theme(false));
    s.start_drawing(0);
    s.submit();
    s.submit_failed("You already submitted a drawing today.".into());
    assert_eq!(s.phase, Phase::Drawing);
    assert_eq!(
        s.error.as_deref(),
        Some("You already submitted a drawing today.")
    );
    // a stale success after the session moved on is ignored
    assert!(s.submit_succeeded("x".into()).is_empty());
    assert!(!s.submitted);
}

#[test]
fn theme_failure_then_retry() {
    let (mut s, _) = Session::open(true);
    s.theme_failed("Could not reach the Daily Skribbl server.".into());
    assert_eq!(s.phase, Phase::Intro);
    assert_eq!(s.theme, "");
    assert!(s.error.is_some());
    assert_eq!(s.retry(), [Effect::LoadTheme]);
    assert!(s.error.is_none());
    let effects = s.theme_loaded(theme(false));
    assert_eq!(effects, [Effect::LoadWinner]);
    // the same date again does not refetch the winner
    s.retry();
    assert!(s.theme_loaded(theme(false)).is_empty());
}

#[test]
fn gallery_pages_append_without_duplicates_and_ignore_failures() {
    let (mut s, _) = Session::open(true);
    s.theme_loaded(theme(true));
    assert!(
        s.load_gallery(0).is_empty(),
        "already loading: the button is disabled"
    );
    s.gallery_loaded(
        0,
        GalleryPage {
            drawings: (0..16)
                .map(|i| drawing(&format!("d{i}"), false, 0, 0))
                .collect(),
            next_offset: Some(16),
            has_more: true,
        },
    );
    assert_eq!(
        (s.gallery.len(), s.gallery_offset, s.gallery_has_more),
        (16, 16, true)
    );
    assert_eq!(s.load_gallery(16), [Effect::LoadGallery { offset: 16 }]);
    // a page overlapping already-shown rows (offsets shifted by a new submission)
    s.gallery_loaded(
        16,
        GalleryPage {
            drawings: vec![drawing("d15", false, 0, 0), drawing("d16", false, 0, 0)],
            next_offset: None,
            has_more: false,
        },
    );
    assert_eq!(s.gallery.len(), 17);
    assert_eq!(s.gallery_offset, 16, "nextOffset ?? offset");
    s.load_gallery(0);
    s.gallery_failed();
    assert!(!s.gallery_loading);
    assert_eq!(s.gallery.len(), 17, "a failure keeps what is shown");
    assert_eq!(s.error, None, "gallery failures are silent");
    // a refresh replaces
    s.load_gallery(0);
    s.gallery_loaded(
        0,
        GalleryPage {
            drawings: vec![],
            next_offset: None,
            has_more: false,
        },
    );
    assert_eq!(s.gallery_count_label(), "0 drawings");
}

#[test]
fn gallery_rows_are_bounded() {
    let (mut s, _) = Session::open(true);
    s.theme_loaded(theme(true));
    s.gallery_loaded(
        0,
        GalleryPage {
            drawings: (0..MAX_GALLERY_ROWS + 50)
                .map(|i| drawing(&format!("d{i}"), false, 0, 0))
                .collect(),
            next_offset: Some(5000),
            has_more: true,
        },
    );
    assert_eq!(s.gallery.len(), MAX_GALLERY_ROWS);
    assert!(!s.gallery_has_more);
}

#[test]
fn votes_toggle_reconcile_and_roll_back() {
    let (mut s, _) = Session::open(true);
    s.theme_loaded(theme(true));
    s.gallery_loaded(
        0,
        GalleryPage {
            drawings: vec![drawing("own", true, 3, 0), drawing("b", false, 5, 0)],
            next_offset: None,
            has_more: false,
        },
    );
    let b = did("b");
    assert!(
        s.cast_vote(&did("own"), 1).is_empty(),
        "cannot vote own drawing"
    );
    assert!(s.cast_vote(&did("missing"), 1).is_empty());
    assert!(s.cast_vote(&b, 2).is_empty(), "only -1/1");
    // up: optimistic +1
    let e = s.cast_vote(&b, 1);
    assert_eq!(
        e,
        [Effect::Vote {
            drawing: b.clone(),
            vote: 1,
            seq: 1
        }]
    );
    assert_eq!(
        (
            s.gallery[1].vote_score,
            s.gallery[1].my_vote,
            s.gallery[1].vote_count
        ),
        (6, 1, 1)
    );
    s.vote_succeeded(&b, 1, 9); // others voted meanwhile: the server's score wins
    assert_eq!(s.gallery[1].vote_score, 9);
    // change to down: -1 for the old up, -1 for the down
    let e = s.cast_vote(&b, -1);
    assert_eq!(
        e,
        [Effect::Vote {
            drawing: b.clone(),
            vote: -1,
            seq: 2
        }]
    );
    assert_eq!(
        (
            s.gallery[1].vote_score,
            s.gallery[1].my_vote,
            s.gallery[1].vote_count
        ),
        (7, -1, 1)
    );
    // the same vote again removes it (0 = neutral)
    let e = s.cast_vote(&b, -1);
    assert_eq!(
        e,
        [Effect::Vote {
            drawing: b.clone(),
            vote: 0,
            seq: 3
        }]
    );
    assert_eq!(
        (
            s.gallery[1].vote_score,
            s.gallery[1].my_vote,
            s.gallery[1].vote_count
        ),
        (8, 0, 0)
    );
    // a delayed reply for seq 2 arrives after seq 3 was sent: ignored
    s.vote_succeeded(&b, 2, 100);
    assert_eq!(s.gallery[1].vote_score, 8);
    // seq 3 fails: roll back to the state before the first unanswered vote (score 9, up)
    s.vote_failed(&b, 3);
    assert_eq!((s.gallery[1].vote_score, s.gallery[1].my_vote), (9, 1));
    assert_eq!(s.error.as_deref(), Some("Vote could not be saved."));
}

#[test]
fn tools_and_lightbox_are_validated() {
    let (mut s, _) = Session::open(true);
    s.set_brush(7);
    assert_eq!(s.brush, DEFAULT_BRUSH);
    s.set_brush(26);
    assert_eq!(s.brush, 26);
    s.set_color(0xff12_3456);
    assert_eq!(s.color, 0x12_3456);
    s.expand(Some(did("nope")));
    assert_eq!(
        s.expanded, None,
        "only a shown drawing can open the lightbox"
    );
    assert_eq!(winner_score_label(1), "1 point");
    assert_eq!(winner_score_label(-2), "-2 points");
    assert_eq!(PALETTE.len(), 21);
}
