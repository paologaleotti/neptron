use std::io::{self, BufRead};

/// The longest request line a server reads, in bytes, without the newline.
/// The largest request, an input script, stays far below it.
pub const MAX_REQUEST_LINE_BYTES: usize = 1 << 20;

/// The longest response line a client reads, in bytes, without the newline.
/// A screenshot of the scanout stage travels as base64 in one line, so the
/// limit leaves room for a 4096x2160 picture with no compression.
pub const MAX_RESPONSE_LINE_BYTES: usize = 64 << 20;

/// What one call of [`read_line`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineRead {
    /// A whole line is in the buffer, without its newline and without a
    /// carriage return before the newline.
    Line,
    /// The peer closed the stream before any byte of a new line.
    End,
    /// The line has more than the limit of bytes. The buffer holds the first
    /// bytes and the rest of the line is still in the stream.
    TooLong,
}

/// Reads one line into `buf`, and never holds more than `max_bytes` bytes of
/// it.
///
/// A line ends at a newline, or at the end of the stream when the peer
/// closes it after the last byte. The function reads only from `reader`, so
/// a test can drive it with a byte slice.
pub fn read_line<R: BufRead + ?Sized>(
    reader: &mut R,
    max_bytes: usize,
    buf: &mut Vec<u8>,
) -> io::Result<LineRead> {
    buf.clear();
    loop {
        let available = match reader.fill_buf() {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if available.is_empty() {
            return Ok(if buf.is_empty() {
                LineRead::End
            } else {
                strip_cr(buf);
                LineRead::Line
            });
        }
        let room = max_bytes - buf.len();
        match available.iter().position(|&b| b == b'\n') {
            Some(end) if end <= room => {
                buf.extend_from_slice(&available[..end]);
                reader.consume(end + 1);
                strip_cr(buf);
                return Ok(LineRead::Line);
            }
            _ if available.len() > room => {
                let take = room;
                buf.extend_from_slice(&available[..take]);
                reader.consume(take);
                return Ok(LineRead::TooLong);
            }
            _ => {
                let take = available.len();
                buf.extend_from_slice(available);
                reader.consume(take);
            }
        }
    }
}

fn strip_cr(buf: &mut Vec<u8>) {
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    fn lines(input: &[u8], max: usize, chunk: usize) -> Vec<(LineRead, Vec<u8>)> {
        let mut reader = BufReader::with_capacity(chunk, input);
        let mut buf = Vec::new();
        let mut out = Vec::new();
        loop {
            let r = read_line(&mut reader, max, &mut buf).unwrap();
            out.push((r, buf.clone()));
            if r != LineRead::Line {
                return out;
            }
        }
    }

    #[test]
    fn splits_at_newlines_for_every_buffer_size() {
        for chunk in 1..12 {
            assert_eq!(
                lines(b"ab\ncde\n\nf", 16, chunk),
                vec![
                    (LineRead::Line, b"ab".to_vec()),
                    (LineRead::Line, b"cde".to_vec()),
                    (LineRead::Line, b"".to_vec()),
                    (LineRead::Line, b"f".to_vec()),
                    (LineRead::End, b"".to_vec()),
                ],
                "chunk {chunk}"
            );
        }
    }

    #[test]
    fn strips_a_carriage_return_before_the_newline() {
        assert_eq!(lines(b"ab\r\n", 16, 8)[0], (LineRead::Line, b"ab".to_vec()));
    }

    #[test]
    fn a_line_of_exactly_the_limit_is_read() {
        for chunk in 1..8 {
            assert_eq!(
                lines(b"abcd\n", 4, chunk)[0],
                (LineRead::Line, b"abcd".to_vec())
            );
            assert_eq!(
                lines(b"abcd", 4, chunk)[0],
                (LineRead::Line, b"abcd".to_vec())
            );
        }
    }

    #[test]
    fn a_line_over_the_limit_stops_at_the_limit() {
        for chunk in 1..8 {
            let out = lines(b"abcde\nf\n", 4, chunk);
            assert_eq!(
                out,
                vec![(LineRead::TooLong, b"abcd".to_vec())],
                "chunk {chunk}"
            );
        }
    }

    #[test]
    fn a_long_line_with_no_newline_holds_only_the_limit() {
        let input = vec![b'x'; 10_000];
        let out = lines(&input, 100, 4096);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, LineRead::TooLong);
        assert_eq!(out[0].1.len(), 100);
    }

    #[test]
    fn an_empty_stream_ends_at_once() {
        assert_eq!(lines(b"", 4, 4), vec![(LineRead::End, Vec::new())]);
    }
}
