use std::fs::{self, OpenOptions};
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use otelo_journal::Hour;

use crate::frame::{FrameRead, read_next_frame};
use crate::maintenance::{compress_segment, delete_segments_past_retention};
use crate::segment::SegmentDirectory;
use crate::signal_log::OpenSegment;

pub struct RecoveredSegments {
    pub open_segment: Option<OpenSegment>,
    pub newest_closed_hour: Option<Hour>,
}

pub fn recover_segments(
    segments: &SegmentDirectory,
    retained_since: i64,
    now: i64,
) -> anyhow::Result<RecoveredSegments> {
    fs::create_dir_all(segments.path())
        .with_context(|| format!("make the directory {}", segments.path().display()))?;
    segments.delete_temporary_files()?;
    for (hour, files) in segments.list_segments()? {
        // The compressed segment is renamed into place only once it is synced.
        if files.uncompressed && files.compressed {
            let path = segments.uncompressed_segment_path(hour);
            fs::remove_file(&path).with_context(|| format!("delete {}", path.display()))?;
        }
    }
    delete_segments_past_retention(segments, &segments.list_segments()?, None, retained_since)?;
    let listed_segments = segments.list_segments()?;
    let newest_hour = listed_segments.keys().next_back().copied();
    let mut open_segment = None;
    let uncompressed_hours = listed_segments
        .iter()
        .filter(|(_, files)| files.uncompressed)
        .map(|(hour, _)| *hour);
    for hour in uncompressed_hours {
        let path = segments.uncompressed_segment_path(hour);
        let length = cut_damaged_tail(&path)?;
        if Some(hour) == newest_hour && hour >= Hour::containing(now) {
            let file = OpenOptions::new()
                .append(true)
                .open(&path)
                .with_context(|| format!("open {}", path.display()))?;
            open_segment = Some(OpenSegment {
                hour,
                file: Arc::new(file),
                length,
            });
        } else {
            compress_segment(segments, hour)?;
        }
    }
    let open_hour = open_segment.as_ref().map(|open: &OpenSegment| open.hour);
    let newest_closed_hour = listed_segments
        .keys()
        .copied()
        .rfind(|hour| Some(*hour) != open_hour);
    Ok(RecoveredSegments {
        open_segment,
        newest_closed_hour,
    })
}

fn cut_damaged_tail(path: &Path) -> anyhow::Result<u64> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;
    let file_length = file
        .metadata()
        .with_context(|| format!("read the size of {}", path.display()))?
        .len();
    let mut frames = BufReader::new(&file);
    let mut length = 0;
    while let FrameRead::Frame { frame_bytes, .. } =
        read_next_frame(&mut frames).with_context(|| format!("read {}", path.display()))?
    {
        length += frame_bytes;
    }
    if length < file_length {
        tracing::warn!(
            "cut {} bytes off the end of {}, the frame a crash left unfinished",
            file_length - length,
            path.display()
        );
        file.set_len(length)
            .and_then(|()| file.sync_all())
            .with_context(|| format!("cut the unfinished frame off {}", path.display()))?;
    }
    Ok(length)
}
