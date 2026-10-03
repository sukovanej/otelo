use std::io::{self, Read};

const LENGTH_BYTES: usize = 4;
const CHECKSUM_BYTES: usize = 4;
const HEADER_BYTES: usize = LENGTH_BYTES + CHECKSUM_BYTES;
const RECEIVED_AT_BYTES: usize = 8;

pub fn encode_frame(received_at: i64, request: &[u8]) -> anyhow::Result<Vec<u8>> {
    let checked_length = u32::try_from(RECEIVED_AT_BYTES + request.len()).map_err(|_| {
        anyhow::anyhow!(
            "a request of {} bytes is too large for a frame",
            request.len()
        )
    })?;
    let mut frame = Vec::with_capacity(HEADER_BYTES + RECEIVED_AT_BYTES + request.len());
    frame.extend_from_slice(&checked_length.to_le_bytes());
    frame.extend_from_slice(&[0; CHECKSUM_BYTES]);
    frame.extend_from_slice(&received_at.to_le_bytes());
    frame.extend_from_slice(request);
    let checksum = crc32c::crc32c(&frame[HEADER_BYTES..]);
    frame[LENGTH_BYTES..HEADER_BYTES].copy_from_slice(&checksum.to_le_bytes());
    Ok(frame)
}

pub enum FrameRead {
    Frame {
        received_at: i64,
        request: Vec<u8>,
        frame_bytes: u64,
    },
    EndOfSegment,
    Damaged,
}

pub fn read_next_frame(segment: &mut impl Read) -> io::Result<FrameRead> {
    let mut header = [0; HEADER_BYTES];
    match read_as_much_as_fits(segment, &mut header)? {
        0 => return Ok(FrameRead::EndOfSegment),
        HEADER_BYTES => {}
        _ => return Ok(FrameRead::Damaged),
    }
    let (length, checksum) = header.split_at(LENGTH_BYTES);
    let checked_length = u32::from_le_bytes(length.try_into().expect("four bytes"));
    let checksum = u32::from_le_bytes(checksum.try_into().expect("four bytes"));
    if (checked_length as usize) < RECEIVED_AT_BYTES {
        return Ok(FrameRead::Damaged);
    }
    // The length may be garbage, so the buffer grows with what the segment holds.
    let mut checked = Vec::new();
    segment
        .take(u64::from(checked_length))
        .read_to_end(&mut checked)?;
    if checked.len() != checked_length as usize || crc32c::crc32c(&checked) != checksum {
        return Ok(FrameRead::Damaged);
    }
    let request = checked.split_off(RECEIVED_AT_BYTES);
    let received_at = i64::from_le_bytes(checked.try_into().expect("eight bytes"));
    Ok(FrameRead::Frame {
        received_at,
        request,
        frame_bytes: (HEADER_BYTES as u64) + u64::from(checked_length),
    })
}

fn read_as_much_as_fits(segment: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match segment.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}
