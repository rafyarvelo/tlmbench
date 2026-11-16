//! The part of the `zstd` crate the `parquet` crate calls, for the browser-only Parquet module (`crates/web-parquet`).
//! The real crate binds to C, which needs a compiler that targets WebAssembly. Here decoding is `ruzstd` (pure Rust) and
//! compressing is not offered: the page only reads files.

use std::io::{self, Cursor, Read};
use std::marker::PhantomData;

pub mod bulk {
    use super::*;

    pub struct Compressor<'a>(PhantomData<&'a ()>);

    impl Compressor<'static> {
        pub fn new(_level: i32) -> io::Result<Self> {
            Ok(Self(PhantomData))
        }
    }

    impl Compressor<'_> {
        pub fn compress_to_buffer(&mut self, _src: &[u8], _dst: &mut Cursor<&mut Vec<u8>>) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "this build only reads Zstandard"))
        }
    }

    pub struct Decompressor<'a>(PhantomData<&'a ()>);

    impl Decompressor<'static> {
        pub fn new() -> io::Result<Self> {
            Ok(Self(PhantomData))
        }
    }

    impl Decompressor<'_> {
        /// Every frame in `src`, written at the cursor's position; the number of bytes it made.
        pub fn decompress_to_buffer(&mut self, src: &[u8], dst: &mut Cursor<&mut Vec<u8>>) -> io::Result<usize> {
            let at = dst.position() as usize;
            let out = dst.get_mut();
            out.truncate(at);
            let mut rest = src;
            while !rest.is_empty() {
                let mut frame = ruzstd::decoding::StreamingDecoder::new(&mut rest)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
                frame.read_to_end(out)?;
            }
            Ok(out.len() - at)
        }
    }
}

pub mod zstd_safe {
    #[derive(Debug)]
    pub struct Unknown;

    /// Left unknown: the caller sizes the output from the page's own header.
    pub fn get_frame_content_size(_src: &[u8]) -> Result<Option<u64>, Unknown> {
        Ok(None)
    }

    pub fn compress_bound(n: usize) -> usize {
        n + n / 128 + 512
    }
}

/// `zstd::stream::read::Decoder`, which the packet reader (`crates/decode/src/packets_core.rs`) decompresses a payload
/// with: every frame in the input, one after another, decoded a few blocks at a time as it is read, so a reader that
/// stops after a limit (the packet reader's 1 GiB) never holds more than that.
pub mod stream {
    pub mod read {
        use std::io::{self, Cursor, Read};

        use ruzstd::decoding::{BlockDecodingStrategy, FrameDecoder};

        pub struct Decoder<R: Read> {
            src: Option<R>,
            input: Cursor<Vec<u8>>,
            frame: FrameDecoder,
            started: bool,
        }

        fn bad(e: impl std::fmt::Display) -> io::Error {
            io::Error::new(io::ErrorKind::InvalidData, e.to_string())
        }

        impl<R: Read> Decoder<R> {
            pub fn new(src: R) -> io::Result<Self> {
                Ok(Self { src: Some(src), input: Cursor::new(Vec::new()), frame: FrameDecoder::new(), started: false })
            }
        }

        impl<R: Read> Read for Decoder<R> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                // the stored payload is read whole (it is in memory already); what it decodes to is made as it is read
                if let Some(mut src) = self.src.take() {
                    src.read_to_end(self.input.get_mut())?;
                }
                if buf.is_empty() {
                    return Ok(0);
                }
                loop {
                    let n = self.frame.read(buf)?;
                    if n > 0 {
                        return Ok(n);
                    }
                    if !self.started || self.frame.is_finished() {
                        if self.input.position() as usize >= self.input.get_ref().len() {
                            return Ok(0);
                        }
                        self.frame.reset(&mut self.input).map_err(bad)?;
                        self.started = true;
                        continue;
                    }
                    self.frame.decode_blocks(&mut self.input, BlockDecodingStrategy::UptoBytes(1 << 16)).map_err(bad)?;
                }
            }
        }
    }
}
