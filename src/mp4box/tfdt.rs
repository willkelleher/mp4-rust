use std::io::{Read, Seek, Write};
use serde::{Serialize};

use crate::mp4box::*;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TfdtBox {
    pub version: u8,
    pub flags: u32,
    pub base_media_decode_time: u32,
}

impl Default for TfdtBox {
    fn default() -> Self {
        TfdtBox {
            version: 0,
            flags: 0,
            base_media_decode_time: 0,
        }
    }
}
impl TfdtBox {
    pub fn get_type(&self) -> BoxType {
        BoxType::TfdtBox
    }

    pub fn get_size(&self) -> u64 {
        HEADER_SIZE + HEADER_EXT_SIZE + 4
    }
}

impl Mp4Box for TfdtBox {
    fn box_type(&self) -> BoxType {
        return self.get_type();
    }

    fn box_size(&self) -> u64 {
        return self.get_size();
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).unwrap())
    }

    fn summary(&self) -> Result<String> {
        let s = format!("");
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for TfdtBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let (version, flags) = read_box_header_ext(reader)?;
        let base_media_decode_time = reader.read_u32::<BigEndian>()?;

        skip_bytes_to(reader, start + size)?;

        Ok(TfdtBox {
            version,
            flags,
            base_media_decode_time,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for TfdtBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;
        write_box_header_ext(writer, self.version, self.flags)?;

        writer.write_u32::<BigEndian>(self.base_media_decode_time)?;

        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_tfdt() {
        let src_box = TfdtBox {
            version: 0,
            flags: 0,
            base_media_decode_time: 6000,
        };
        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::TfdtBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = TfdtBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}