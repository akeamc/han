use core::str::FromStr;
#[cfg(feature = "embedded-io-async")]
use embedded_io_async::BufRead;

use crate::{obis::Object, Error, Result};

/// A reader for the raw UART output of a power meter.
pub struct Reader<I>
where
    I: Iterator<Item = u8>,
{
    iter: I,
}

impl<I> Reader<I>
where
    I: Iterator<Item = u8>,
{
    /// Construct a new reader from a byte iterator.
    pub fn new(iter: I) -> Self {
        Self { iter }
    }
}

impl<I> Iterator for Reader<I>
where
    I: Iterator<Item = u8>,
{
    type Item = Readout;

    fn next(&mut self) -> Option<Self::Item> {
        while self.iter.next()? != b'/' {}

        let mut buffer = [0u8; 2048];
        buffer[0] = b'/';

        let mut i = 1;
        let mut write = |b| {
            if i >= buffer.len() {
                return None; // Buffer overflow.
            }

            buffer[i] = b;
            i += 1;

            Some(())
        };

        loop {
            let b = self.iter.next()?;
            write(b)?;

            if b == b'!' {
                // Add CRC bytes
                for _ in 0..4 {
                    write(self.iter.next()?)?;
                }

                return Some(Readout { buffer });
            }
        }
    }
}

#[cfg(feature = "embedded-io-async")]
#[derive(Debug)]
struct Buffer {
    data: [u8; 2048],
    pos: usize,
    len: Option<usize>,
}

#[cfg(feature = "embedded-io-async")]
impl Buffer {
    const fn new() -> Self {
        Self {
            data: [0; 2048],
            pos: 0,
            len: None,
        }
    }
}

/// A parser for the raw UART output of a power meter.
#[cfg(feature = "embedded-io-async")]
pub struct AsyncReader<R> {
    reader: R,
    buffer: Option<Buffer>,
}

#[cfg(feature = "embedded-io-async")]
async fn scan_to_next<R>(reader: &mut R) -> Result<Option<Buffer>, R::Error>
where
    R: BufRead,
{
    loop {
        let buf = reader.fill_buf().await?;
        let n = buf.len();
        if n == 0 {
            return Ok(None);
        }

        if let Some(start) = buf.iter().position(|b| *b == b'/') {
            reader.consume(start);
            return Ok(Some(Buffer::new()));
        } else {
            reader.consume(n);
        }
    }
}

#[cfg(feature = "embedded-io-async")]
impl<R> AsyncReader<R>
where
    R: BufRead,
{
    /// Construct a new AsyncReader from a byte reader.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: None,
        }
    }

    /// Read the next readout from the reader.
    ///
    /// This function is cancel-safe.
    pub async fn next_readout(&mut self) -> Result<Option<Readout>, R::Error> {
        loop {
            match self.buffer {
                Some(ref mut buffer) => 'fill_buf: loop {
                    let buf = self.reader.fill_buf().await?;
                    let n = buf.len();

                    if n == 0 {
                        return Ok(None);
                    }

                    for (i, &b) in buf.iter().enumerate() {
                        if buffer.pos >= buffer.data.len() {
                            self.reader.consume(i);
                            self.buffer = None;
                            break 'fill_buf; // buffer overflow
                        }

                        buffer.data[buffer.pos] = b;

                        if buffer.len.is_some_and(|len| buffer.pos >= len) {
                            self.reader.consume(i);
                            let readout = Readout {
                                buffer: buffer.data,
                            };
                            self.buffer = None;
                            return Ok(Some(readout));
                        }

                        // end of telegram 4 bytes after the '!'
                        if b == b'!' {
                            buffer.len = Some(buffer.pos + 4);
                        }

                        buffer.pos += 1;
                    }

                    self.reader.consume(n);
                },
                None => match scan_to_next(&mut self.reader).await? {
                    Some(buffer) => {
                        self.buffer = Some(buffer);
                    }
                    None => return Ok(None),
                },
            }
        }
    }
}

/// A single readout.
pub struct Readout {
    pub(crate) buffer: [u8; 2048],
}

impl Readout {
    /// Attempt to parse this as a [`Telegram`].
    pub fn to_telegram(&self) -> Result<Telegram<'_>> {
        let buffer = core::str::from_utf8(&self.buffer).map_err(|_| Error::InvalidFormat)?;
        let end = buffer.find('!').ok_or(Error::InvalidFormat)?;
        let (buffer, postfix) = buffer.split_at(end + 1);
        let received_checksum =
            u16::from_str_radix(postfix.get(..4).ok_or(Error::InvalidFormat)?, 16)
                .map_err(|_| Error::InvalidFormat)?;
        let checksum = crc16::State::<crc16::ARC>::calculate(buffer.as_bytes());

        if received_checksum != checksum {
            return Err(Error::Checksum);
        }

        let (header, body) = buffer.split_once("\r\n\r\n").ok_or(Error::InvalidFormat)?;

        Ok(Telegram {
            checksum,
            flag_id: header.get(1..4).ok_or(Error::InvalidFormat)?,
            identification: header.get(5..).ok_or(Error::InvalidFormat)?,
            object_buffer: body
                .get(..body.len().checked_sub(3).ok_or(Error::InvalidFormat)?)
                .ok_or(Error::InvalidFormat)?,
        })
    }
}

/// A single telegram.
pub struct Telegram<'a> {
    /// CRC16 checksum.
    pub checksum: u16,
    /// 3-letter [FLAG ID](https://www.dlms.com/eng/flag-id-list-44143.shtml)
    /// identifying the manufacturer.
    pub flag_id: &'a str,
    /// Power meter ID.
    pub identification: &'a str,
    object_buffer: &'a str,
}

impl<'a> Telegram<'a> {
    /// Iterator of the data containedby the telegram.
    pub fn objects(&self) -> impl Iterator<Item = Result<Object>> + 'a {
        self.object_buffer.lines().map(Object::from_str)
    }
}

#[cfg(test)]
mod tests {
    use super::Reader;

    #[test]
    fn ellevio() {
        let bytes = include_bytes!("../test/ell.txt");
        let mut reader = Reader::new(bytes.iter().cloned());
        let readout = reader.next().unwrap();
        let telegram = readout.to_telegram().unwrap();

        assert_eq!(telegram.checksum, 0x9ab5);
        assert_eq!(telegram.flag_id, "ELL");
        assert_eq!(telegram.identification, "\\253833635_A");

        for obj in telegram.objects() {
            obj.unwrap();
        }

        assert!(reader.next().is_none());
    }

    #[cfg(feature = "embedded-io-async")]
    #[tokio::test]
    async fn ellevio_async() {
        let bytes = include_bytes!("../test/ell.txt");
        let mut reader = super::AsyncReader::new(&bytes[..]);
        let readout = reader.next_readout().await.unwrap().unwrap();
        let telegram = readout.to_telegram().unwrap();

        assert_eq!(telegram.checksum, 0x9ab5);
        assert_eq!(telegram.flag_id, "ELL");
        assert_eq!(telegram.identification, "\\253833635_A");

        for obj in telegram.objects() {
            obj.unwrap();
        }

        assert!(reader.next_readout().await.unwrap().is_none());
    }
}
