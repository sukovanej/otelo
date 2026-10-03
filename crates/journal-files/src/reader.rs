use std::collections::{BTreeMap, VecDeque};
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::PathBuf;

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
        let first_hour = from.map(|position| position.hour);
        let planned_segments = listed_segments
            .keys()
            .copied()
            .filter(|hour| first_hour.is_none_or(|first_hour| *hour >= first_hour))
            .map_while(|hour| {
                let readable_bytes = match readable_end {
                    ReadableEnd::EveryHour => None,
                    ReadableEnd::OpenSegment(end) if hour < end.hour => None,
                    ReadableEnd::OpenSegment(end) if hour == end.hour => Some(end.offset),
                    ReadableEnd::OpenSegment(_) => return None,
                };
                let skipped_bytes = from
                    .filter(|position| position.hour == hour)
                    .map_or(0, |position| position.offset);
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
                            hour: segment.hour,
                            offset: segment.offset,
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
// same bytes, so whichever exists when it opens is read.
fn open_planned_segment(planned: PlannedSegment) -> anyhow::Result<SegmentReader> {
    let (path, segment): (PathBuf, Box<dyn Read + Send>) =
        match File::open(&planned.uncompressed_path) {
            Ok(file) => (planned.uncompressed_path, Box::new(file)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let file = File::open(&planned.compressed_path)
                    .with_context(|| format!("open {}", planned.compressed_path.display()))?;
                let decoder = zstd::Decoder::new(file)
                    .with_context(|| format!("decompress {}", planned.compressed_path.display()))?;
                (planned.compressed_path, Box::new(decoder))
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("open {}", planned.uncompressed_path.display()));
            }
        };
    let mut segment: Box<dyn Read + Send> = match planned.readable_bytes {
        Some(readable_bytes) => Box::new(segment.take(readable_bytes)),
        None => segment,
    };
    let skipped = io::copy(
        &mut segment.by_ref().take(planned.skipped_bytes),
        &mut io::sink(),
    )
    .with_context(|| format!("read {}", path.display()))?;
    if skipped < planned.skipped_bytes {
        return Err(anyhow!(
            "{} ends at byte {skipped}, before the position {}",
            path.display(),
            planned.skipped_bytes
        ));
    }
    Ok(SegmentReader {
        hour: planned.hour,
        path,
        frames: BufReader::new(segment),
        offset: planned.skipped_bytes,
    })
}
