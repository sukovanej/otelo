use std::fs::{self, OpenOptions};
use std::io::Write;
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use otelo_journal::{Frame, Hour, Journal, Position, SyncedEnd};
use otelo_journal_files::{Config, JournalFiles, JournalThreads};
use otelo_query::Signal;

const NANOS_PER_HOUR: i64 = 3_600 * 1_000_000_000;
const NANOS_PER_DAY: i64 = 24 * NANOS_PER_HOUR;
const RETENTION_DAYS: NonZeroU16 = NonZeroU16::new(30).expect("thirty is not zero");

fn now() -> i64 {
    i64::try_from(jiff::Timestamp::now().as_nanosecond()).unwrap()
}

fn still_open_after_a_restart() -> i64 {
    now() + 2 * NANOS_PER_HOUR
}

fn open_journal(directory: &Path) -> (Arc<JournalFiles>, JournalThreads) {
    let opened = JournalFiles::open(Config::new(directory.to_owned(), RETENTION_DAYS)).unwrap();
    (opened.journal, opened.threads)
}

async fn append_log_request(journal: &JournalFiles, received_at: i64, request: &[u8]) {
    journal
        .append_frame(Signal::Logs, received_at, request)
        .unwrap()
        .wait_until_synced()
        .await
        .unwrap();
}

fn read_log_frames(journal: &JournalFiles, from: Option<Position>) -> Vec<Frame> {
    journal
        .read_frames(Signal::Logs, from)
        .unwrap()
        .collect::<anyhow::Result<_>>()
        .unwrap()
}

fn requests_of(frames: &[Frame]) -> Vec<&[u8]> {
    frames
        .iter()
        .map(|frame| frame.request.as_slice())
        .collect()
}

fn log_segment_path(directory: &Path, received_at: i64, extension: &str) -> PathBuf {
    let hour = jiff::Timestamp::from_nanosecond(i128::from(received_at))
        .unwrap()
        .strftime("%Y-%m-%dT%H");
    directory.join("logs").join(format!("{hour}.{extension}"))
}

#[tokio::test]
async fn frame_holds_its_length_checksum_receipt_and_request() {
    let directory = tempfile::tempdir().unwrap();
    let received_at = now();
    let (journal, threads) = open_journal(directory.path());

    append_log_request(&journal, received_at, b"request").await;
    threads.stop_and_join().unwrap();

    let segment = fs::read(log_segment_path(directory.path(), received_at, "seg")).unwrap();
    let checked = [received_at.to_le_bytes().as_slice(), b"request"].concat();
    let expected = [
        u32::try_from(checked.len())
            .unwrap()
            .to_le_bytes()
            .as_slice(),
        crc32c::crc32c(&checked).to_le_bytes().as_slice(),
        &checked,
    ]
    .concat();
    assert_eq!(segment, expected);
}

#[tokio::test]
async fn frames_are_read_back_from_the_open_and_the_compressed_segments() {
    let directory = tempfile::tempdir().unwrap();
    let now = now();
    let two_hours_ago = now - 2 * NANOS_PER_HOUR;
    let (journal, threads) = open_journal(directory.path());

    append_log_request(&journal, two_hours_ago, b"first").await;
    append_log_request(&journal, two_hours_ago + 1, b"second").await;
    append_log_request(&journal, now, b"third").await;
    let frames_before_restart = read_log_frames(&journal, None);
    threads.stop_and_join().unwrap();
    let (journal, threads) = open_journal(directory.path());
    let frames_after_restart = read_log_frames(&journal, None);
    threads.stop_and_join().unwrap();

    assert_eq!(
        requests_of(&frames_before_restart),
        [b"first".as_slice(), b"second", b"third"]
    );
    assert_eq!(frames_after_restart, frames_before_restart);
    assert_eq!(frames_after_restart[1].received_at, two_hours_ago + 1);
    assert_eq!(
        frames_after_restart[2].position_after.segment_hour,
        Hour::containing(now)
    );
    assert!(log_segment_path(directory.path(), two_hours_ago, "seg.zst").exists());
    assert!(!log_segment_path(directory.path(), two_hours_ago, "seg").exists());
}

#[tokio::test]
async fn unfinished_last_frame_is_cut_off_at_startup() {
    let directory = tempfile::tempdir().unwrap();
    let received_at = still_open_after_a_restart();
    let (journal, threads) = open_journal(directory.path());
    append_log_request(&journal, received_at, b"first").await;
    append_log_request(&journal, received_at, b"second").await;
    threads.stop_and_join().unwrap();
    let segment_path = log_segment_path(directory.path(), received_at, "seg");
    let complete_length = fs::metadata(&segment_path).unwrap().len();
    OpenOptions::new()
        .append(true)
        .open(&segment_path)
        .unwrap()
        .write_all(&[20, 0, 0, 0, 1, 2])
        .unwrap();

    let (journal, threads) = open_journal(directory.path());
    append_log_request(&journal, received_at, b"third").await;
    let frames = read_log_frames(&journal, None);
    threads.stop_and_join().unwrap();

    assert_eq!(
        requests_of(&frames),
        [b"first".as_slice(), b"second", b"third"]
    );
    assert_eq!(frames[1].position_after.byte_offset, complete_length);
}

#[tokio::test]
async fn last_frame_with_a_wrong_checksum_is_cut_off_at_startup() {
    let directory = tempfile::tempdir().unwrap();
    let received_at = still_open_after_a_restart();
    let (journal, threads) = open_journal(directory.path());
    append_log_request(&journal, received_at, b"first").await;
    append_log_request(&journal, received_at, b"second").await;
    threads.stop_and_join().unwrap();
    let segment_path = log_segment_path(directory.path(), received_at, "seg");
    let mut segment = fs::read(&segment_path).unwrap();
    *segment.last_mut().unwrap() ^= 0xff;
    fs::write(&segment_path, segment).unwrap();

    let (journal, threads) = open_journal(directory.path());
    let frames = read_log_frames(&journal, None);
    threads.stop_and_join().unwrap();

    assert_eq!(requests_of(&frames), [b"first".as_slice()]);
    assert_eq!(
        fs::metadata(&segment_path).unwrap().len(),
        frames[0].position_after.byte_offset
    );
}

#[tokio::test]
async fn uncompressed_segment_of_a_past_hour_is_compressed_at_startup() {
    let directory = tempfile::tempdir().unwrap();
    let two_hours_ago = now() - 2 * NANOS_PER_HOUR;
    let (journal, threads) = open_journal(directory.path());
    append_log_request(&journal, two_hours_ago, b"first").await;
    threads.stop_and_join().unwrap();
    assert!(log_segment_path(directory.path(), two_hours_ago, "seg").exists());

    let (journal, threads) = open_journal(directory.path());
    let frames = read_log_frames(&journal, None);
    threads.stop_and_join().unwrap();

    assert!(!log_segment_path(directory.path(), two_hours_ago, "seg").exists());
    assert!(log_segment_path(directory.path(), two_hours_ago, "seg.zst").exists());
    assert_eq!(requests_of(&frames), [b"first".as_slice()]);
}

#[tokio::test]
async fn reader_resumes_from_a_position_across_the_end_of_an_hour() {
    let directory = tempfile::tempdir().unwrap();
    let now = now();
    let two_hours_ago = now - 2 * NANOS_PER_HOUR;
    let (journal, threads) = open_journal(directory.path());
    append_log_request(&journal, two_hours_ago, b"first").await;
    let position = read_log_frames(&journal, None)[0].position_after;

    append_log_request(&journal, now, b"second").await;
    let frames_after_position = read_log_frames(&journal, Some(position));
    threads.stop_and_join().unwrap();

    assert_eq!(requests_of(&frames_after_position), [b"second".as_slice()]);
}

#[tokio::test]
async fn reader_resumes_from_a_position_in_a_segment_compressed_since() {
    let directory = tempfile::tempdir().unwrap();
    let now = now();
    let two_hours_ago = now - 2 * NANOS_PER_HOUR;
    let (journal, threads) = open_journal(directory.path());
    append_log_request(&journal, two_hours_ago, b"first").await;
    append_log_request(&journal, two_hours_ago, b"second").await;
    let position = read_log_frames(&journal, None)[0].position_after;
    append_log_request(&journal, now, b"third").await;
    threads.stop_and_join().unwrap();
    assert!(log_segment_path(directory.path(), two_hours_ago, "seg.zst").exists());

    let (journal, threads) = open_journal(directory.path());
    let frames_after_position = read_log_frames(&journal, Some(position));
    threads.stop_and_join().unwrap();

    assert_eq!(
        requests_of(&frames_after_position),
        [b"second".as_slice(), b"third"]
    );
}

#[tokio::test]
async fn frame_received_before_the_open_hour_goes_to_the_open_hour() {
    let directory = tempfile::tempdir().unwrap();
    let now = now();
    let (journal, threads) = open_journal(directory.path());

    append_log_request(&journal, now, b"first").await;
    append_log_request(&journal, now - 2 * NANOS_PER_HOUR, b"second").await;
    let frames = read_log_frames(&journal, None);
    threads.stop_and_join().unwrap();

    assert_eq!(
        frames
            .iter()
            .map(|frame| frame.position_after.segment_hour)
            .collect::<Vec<_>>(),
        [Hour::containing(now), Hour::containing(now)]
    );
}

#[tokio::test]
async fn startup_deletes_the_segments_past_the_retention() {
    let directory = tempfile::tempdir().unwrap();
    let now = now();
    let forty_days_ago = now - 40 * NANOS_PER_DAY;
    let (journal, threads) = open_journal(directory.path());
    append_log_request(&journal, forty_days_ago, b"expired").await;
    append_log_request(&journal, now - 2 * NANOS_PER_DAY, b"retained").await;
    threads.stop_and_join().unwrap();

    let (journal, threads) = open_journal(directory.path());
    let frames = read_log_frames(&journal, None);
    threads.stop_and_join().unwrap();

    assert_eq!(requests_of(&frames), [b"retained".as_slice()]);
    assert!(!log_segment_path(directory.path(), forty_days_ago, "seg.zst").exists());
}

#[tokio::test]
async fn size_adds_up_the_segments_of_every_signal() {
    let directory = tempfile::tempdir().unwrap();
    let received_at = now();
    let (journal, threads) = open_journal(directory.path());

    append_log_request(&journal, received_at, b"log").await;
    journal
        .append_frame(Signal::Metrics, received_at, b"metric")
        .unwrap()
        .wait_until_synced()
        .await
        .unwrap();
    let size_in_bytes = journal.size_in_bytes().unwrap();
    threads.stop_and_join().unwrap();

    assert_eq!(size_in_bytes, 2 * 16 + 3 + 6);
}

#[tokio::test]
async fn each_sync_sends_its_end_to_the_indexer() {
    let directory = tempfile::tempdir().unwrap();
    let opened =
        JournalFiles::open(Config::new(directory.path().to_owned(), RETENTION_DAYS)).unwrap();
    let received_at = now();
    append_log_request(&opened.journal, received_at, b"first").await;
    let frames = read_log_frames(&opened.journal, None);
    let synced_end = opened
        .synced_ends
        .wait_for_synced_end(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(
        synced_end,
        SyncedEnd {
            signal: Signal::Logs,
            end: frames[0].position_after,
            newest_received_at: received_at,
        }
    );
    opened.threads.stop_and_join().unwrap();
}
