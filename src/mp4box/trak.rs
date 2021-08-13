use serde::Serialize;
use std::io::{Read, Seek, SeekFrom, Write};
use std::str::FromStr;

use crate::mp4box::*;
use crate::mp4box::{edts::EdtsBox, hdlr::HdlrBox, mdia::MdiaBox, tkhd::TkhdBox};

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct TrakBox {
    pub tkhd: TkhdBox,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub edts: Option<EdtsBox>,

    pub mdia: MdiaBox,
}

impl TrakBox {
    pub fn get_type(&self) -> BoxType {
        BoxType::TrakBox
    }

    pub fn get_size(&self) -> u64 {
        let mut size = HEADER_SIZE;
        size += self.tkhd.box_size();
        if let Some(ref edts) = self.edts {
            size += edts.box_size();
        }
        size += self.mdia.box_size();
        size += 61;
        size
    }
}

impl Mp4Box for TrakBox {
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

impl<R: Read + Seek> ReadBox<&mut R> for TrakBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let mut tkhd = None;
        let mut edts = None;
        let mut mdia = None;

        let mut current = reader.seek(SeekFrom::Current(0))?;
        let end = start + size;
        while current < end {
            // Get box header.
            let header = BoxHeader::read(reader)?;
            let BoxHeader { name, size: s } = header;

            match name {
                BoxType::TkhdBox => {
                    tkhd = Some(TkhdBox::read_box(reader, s)?);
                }
                BoxType::EdtsBox => {
                    edts = Some(EdtsBox::read_box(reader, s)?);
                }
                BoxType::MdiaBox => {
                    mdia = Some(MdiaBox::read_box(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    skip_box(reader, s)?;
                }
            }

            current = reader.seek(SeekFrom::Current(0))?;
        }

        if tkhd.is_none() {
            return Err(Error::BoxNotFound(BoxType::TkhdBox));
        }
        if mdia.is_none() {
            return Err(Error::BoxNotFound(BoxType::MdiaBox));
        }

        skip_bytes_to(reader, start + size)?;

        Ok(TrakBox {
            tkhd: tkhd.unwrap(),
            edts,
            mdia: mdia.unwrap(),
        })
    }
}

impl<W: Write> WriteBox<&mut W> for TrakBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        self.tkhd.write_box(writer)?;
        if let Some(ref edts) = self.edts {
            edts.write_box(writer)?;
        }
        self.mdia.write_box(writer)?;

        let udta_size = 53;
        BoxHeader::new(BoxType::UdtaBox, HEADER_SIZE + udta_size).write(writer)?;

        let meta_size = 41;
        BoxHeader::new(BoxType::MetaBox, HEADER_SIZE + HEADER_EXT_SIZE + meta_size)
            .write(writer)?;
        write_box_header_ext(writer, 0, 0)?;

        let hdlr = HdlrBox {
            version: 0,
            flags: 0,
            handler_what: FourCC::from_str("mhlr").unwrap(),
            handler_type: FourCC::from_str("mdir").unwrap(),
            name: "".to_owned(),
        };
        hdlr.write_box(writer)?;

        let ilst = BoxHeader::new(BoxType::IlstBox, HEADER_SIZE);
        ilst.write(writer)?;

        Ok(size)
    }
}
