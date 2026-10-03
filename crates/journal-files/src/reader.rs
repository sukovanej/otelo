use std::collections::{BTreeMap, VecDeque};
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use otelo_journal::{Frame, Hour, Position};

use crate::frame::{FrameRead, read_next_frame};
use crate::segment::{SegmentDirectory, SegmentFiles};

#[derive(Clone, Copy, Debug)]
pub enum ReadableEnd {
    EveryHour,
    OpenSegment(Position),
}

struct PlannedSegment {
    hour: Hour,
    uncompressed_path: PathBuf,
    compressed_path: PathBuf,
    skipped_bytes: u64,
    readable_bytes: Option<u64>,
}

struct SegmentReader {
    hour: Hour,
    path: PathBuf,
    frames: BufReader<Box<dyn Read + Send>>,
    offset: u64,
}

pub struct FrameReader {
    planned_segments: VecDeque<PlannedSegment>,
    segment: Option<SegmentReader>,
    failed: bool,
}

impl FrameReader {
    pub fn new(
        segments: &SegmentDirectory,
        listed_segments: &BTreeMap<Hour, SegmentFiles>,
        from: Option<Position>,
        readable_end: ReadableEnd,
    ) -> Self {
        let first_hour = from.map(|position| position.segment_hour);
        let planned_segments = listed_segments
            .keys()
            .copied()
            .filter(|hour| first_hour.is_none_or(|first_hour| *hour >= first_hour))
            .map_while(|hour| {
                let readable_bytes = match readable_end {
                    ReadableEnd::EveryHour => None,
                    ReadableEnd::OpenSegment(end) if hour < end.segment_hour => None,
                    ReadableEnd::OpenSegment(end) if hour == end.segment_hour => {
                        Some(end.byte_offset)
                    }
                    ReadableEnd::OpenSegment(_) => return None,
                };
                let skipped_bytes = from
                    .filter(|position| position.segment_hour == hour)
                    .map_or(0, |position| position.byte_offset);
                Some(PlannedSegment {
                    hour,
                    uncompressed_path: segments.uncompressed_segment_path(hour),
                    compressed_path: segments.compressed_segment_path(hour),
                    skipped_bytes,
                    readable_bytes,
                })
            })
            .collect();
        Self {
            planned_segments,
            segment: None,
            failed: false,
        }
    }

    fn read_next_frame(&mut self) -> anyhow::Result<Option<Frame>> {
        loop {
            let segment = match &mut self.segment {
                Some(segment) => segment,
                None => match self.planned_segments.pop_front() {
                    Some(planned) => self.segment.insert(open_planned_segment(planned)?),
                    None => return Ok(None),
                },
            };
            let frame_read = read_next_frame(&mut segment.frames)
                .with_context(|| format!("read {}", segment.path.display()))?;
            match frame_read {
                FrameRead::Frame {
                    received_at,
                    request,
                    frame_bytes,
                } => {
                    segment.offset += frame_bytes;
                    return Ok(Some(Frame {
                        received_at,
                        request,
                        position_after: Position {
                            segment_hour: segment.hour,
                            byte_offset: segment.offset,
                        },
                    }));
                }
                FrameRead::EndOfSegment => self.segment = None,
                FrameRead::Damaged => {
                    return Err(anyhow!(
                        "{} has a damaged frame at byte {}",
                        segment.path.display(),
                        segment.offset
                    ));
                }
            }
        }
    }
}

impl Iterator for FrameReader {
    type Item = anyhow::Result<Frame>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let read = self.read_next_frame().transpose();
        self.failed = matches!(read, Some(Err(_)));
        read
    }
}

// The compression can replace the uncompressed segment at any moment, and both files hold the
// same bytes, so whichever exists when it opens is read. An indexer that caught up reads from the
// end of the open segment, so that one seeks to the position rather than read up to it.
fn open_planned_segment(planned: PlannedSegment) -> anyhow::Result<SegmentReader> {
    let (path, segment): (PathBuf, Box<dyn Read + Send>) =
        match File::open(&planned.uncompressed_path) {
            Ok(mut file) => {
                let path = planned.uncompressed_path;
                let length = file
                    .metadata()
                    .with_context(|| format!("read the size of {}", path.display()))?
                    .len();
                if length < planned.skipped_bytes {
                    return Err(ended_before_position(&path, length, planned.skipped_bytes));
                }
                file.seek(SeekFrom::Start(planned.skipped_bytes))
                    .with_context(|| format!("read {}", path.display()))?;
                (path, Box::new(file))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let path = planned.compressed_path;
                let file = File::open(&path).with_context(|| format!("open {}", path.display()))?;
                let mut decoder = zstd::Decoder::new(file)
                    .with_context(|| format!("decompress {}", path.display()))?;
                let skipped = io::copy(
                    &mut decoder.by_ref().take(planned.skipped_bytes),
                    &mut io::sink(),
                )
                .with_context(|| format!("read {}", path.display()))?;
                if skipped < planned.skipped_bytes {
                    return Err(ended_before_position(&path, skipped, planned.skipped_bytes));
                }
                (path, Box::new(decoder))
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("open {}", planned.uncompressed_path.display()));
            }
        };
    let segment: Box<dyn Read + Send> = match planned.readable_bytes {
        Some(readable_bytes) if readable_bytes < planned.skipped_bytes => {
            return Err(anyhow!(
                "the position {} is past the synced end {readable_bytes} of {}",
                planned.skipped_bytes,
                path.display()
            ));
        }
        Some(readable_bytes) => Box::new(segment.take(readable_bytes - planned.skipped_bytes)),
        None => segment,
    };
    Ok(SegmentReader {
        hour: planned.hour,
        path,
        frames: BufReader::new(segment),
        offset: planned.skipped_bytes,
    })
}

fn ended_before_position(path: &Path, length: u64, position: u64) -> anyhow::Error {
    anyhow!(
        "{} ends at byte {length}, before the position {position}",
        path.display()
    )
}
