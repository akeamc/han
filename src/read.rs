use core::str::FromStr;

use crate::{obis::Object, Error, Result};

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

pub struct Readout {
    buffer: [u8; 2048],
}

impl Readout {
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
        let prefix = header.get(1..4).ok_or(Error::InvalidFormat)?;
        let identification = header.get(5..).ok_or(Error::InvalidFormat)?;
        Ok(Telegram {
            checksum,
            prefix,
            identification,
            object_buffer: body.get(..body.len() - 3).ok_or(Error::InvalidFormat)?,
        })
    }
}

pub struct Telegram<'a> {
    pub checksum: u16,
    pub prefix: &'a str,
    pub identification: &'a str,
    object_buffer: &'a str,
}

impl<'a> Telegram<'a> {
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
        assert_eq!(telegram.prefix, "ELL");
        assert_eq!(telegram.identification, "\\253833635_A");

        for obj in telegram.objects() {
            obj.unwrap();
        }

        assert!(reader.next().is_none());
    }
}
