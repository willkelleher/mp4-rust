use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Seek, Write};
use serde::{Serialize};

use crate::mp4box::*;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MvhdBox {
    pub version: u8,
    pub flags: u32,
    pub creation_time: u64,
    pub modification_time: u64,
    pub timescale: u32,
    pub duration: u64,

    #[serde(with = "value_u32")]
    pub rate: FixedPointU16,
    pub matrix: tkhd::Matrix,
}

impl MvhdBox {
    pub fn get_type(&self) -> BoxType {
        BoxType::MvhdBox
    }

    pub fn get_size(&self) -> u64 {
        let mut size = HEADER_SIZE + HEADER_EXT_SIZE;
        if self.version == 1 {
            size += 28;
        } else if self.version == 0 {
            size += 16;
        }
        size += 80;
        size
    }
}

impl Default for MvhdBox {
    fn default() -> Self {
        MvhdBox {
            version: 0,
            flags: 0,
            creation_time: 0,
            modification_time: 0,
            timescale: 1000,
            duration: 0,
            rate: FixedPointU16::new(1),
            matrix: tkhd::Matrix::default(),
        }
    }
}

impl Mp4Box for MvhdBox {
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
        let s = format!("creation_time={} timescale={} duration={} rate={}",
            self.creation_time, self.timescale, self.duration, self.rate.value());
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for MvhdBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let (version, flags) = read_box_header_ext(reader)?;

        let (creation_time, modification_time, timescale, duration) = if version == 1 {
            (
                reader.read_u64::<BigEndian>()?,
                reader.read_u64::<BigEndian>()?,
                reader.read_u32::<BigEndian>()?,
                reader.read_u64::<BigEndian>()?,
            )
        } else if version == 0 {
            (
                reader.read_u32::<BigEndian>()? as u64,
                reader.read_u32::<BigEndian>()? as u64,
                reader.read_u32::<BigEndian>()?,
                reader.read_u32::<BigEndian>()? as u64,
            )
        } else {
            return Err(Error::InvalidData("version must be 0 or 1"));
        };
        let rate = FixedPointU16::new_raw(reader.read_u32::<BigEndian>()?);

        skip_bytes_to(reader, start + size)?;

        Ok(MvhdBox {
            version,
            flags,
            creation_time,
            modification_time,
            timescale,
            duration,
            matrix: tkhd::Matrix::default(),
            rate,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for MvhdBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        write_box_header_ext(writer, self.version, self.flags)?;

        if self.version == 1 {
            writer.write_u64::<BigEndian>(self.creation_time)?;
            writer.write_u64::<BigEndian>(self.modification_time)?;
            writer.write_u32::<BigEndian>(self.timescale)?;
            writer.write_u64::<BigEndian>(self.duration)?;
        } else if self.version == 0 {
            writer.write_u32::<BigEndian>(self.creation_time as u32)?;
            writer.write_u32::<BigEndian>(self.modification_time as u32)?;
            writer.write_u32::<BigEndian>(self.timescale)?;
            writer.write_u32::<BigEndian>(self.duration as u32)?;
        } else {
            return Err(Error::InvalidData("version must be 0 or 1"));
        }
        writer.write_u32::<BigEndian>(self.rate.raw_value())?;

        // skip over a bunch of stuff here
        // volume: 2 bytes
        // 2 bytes reserved
        // 8 bytes reserved
        write_zeros(writer, 12)?;

        // 9*4=36 bytes for the matrix
        writer.write_i32::<BigEndian>(self.matrix.a)?;
        writer.write_i32::<BigEndian>(self.matrix.b)?;
        writer.write_i32::<BigEndian>(self.matrix.u)?;
        writer.write_i32::<BigEndian>(self.matrix.c)?;
        writer.write_i32::<BigEndian>(self.matrix.d)?;
        writer.write_i32::<BigEndian>(self.matrix.v)?;
        writer.write_i32::<BigEndian>(self.matrix.x)?;
        writer.write_i32::<BigEndian>(self.matrix.y)?;
        writer.write_i32::<BigEndian>(self.matrix.w)?;

        // 4 bytes preview_time
        // 4 bytes preview_duration
        // 4 bytes poster_time
        // 4 bytes selection_time
        // 4 bytes selection_duration
        // 4 bytes current_time
        // 4 bytes next_track_id
        write_zeros(writer, 28)?;

        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_mvhd32() {
        let src_box = MvhdBox {
            version: 0,
            flags: 0,
            creation_time: 100,
            modification_time: 200,
            timescale: 1000,
            duration: 634634,
            rate: FixedPointU16::new(1),
        };
        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::MvhdBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = MvhdBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_mvhd64() {
        let src_box = MvhdBox {
            version: 1,
            flags: 0,
            creation_time: 100,
            modification_time: 200,
            timescale: 1000,
            duration: 634634,
            rate: FixedPointU16::new(1),
        };
        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::MvhdBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = MvhdBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
