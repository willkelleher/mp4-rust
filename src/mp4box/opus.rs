use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;
use std::io::{Read, Seek, Write};

use crate::mp4box::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpusBox {
    pub data_reference_index: u16,
    pub channelcount: u16,
    pub samplesize: u16,

    #[serde(with = "value_u32")]
    pub samplerate: FixedPointU16,
    pub dops: DopsBox,
}

impl Default for OpusBox {
    fn default() -> Self {
        Self {
            data_reference_index: 0,
            channelcount: 2,
            samplesize: 16,
            samplerate: FixedPointU16::new(48000),
            dops: DopsBox::default(),
        }
    }
}

impl OpusBox {
    pub fn new(config: &OpusConfig) -> Self {
        Self {
            data_reference_index: 1,
            channelcount: config.chan_conf as u16,
            samplesize: 16,
            samplerate: FixedPointU16::new(config.freq_index.freq() as u16),
            dops: DopsBox::new(config),
        }
    }

    pub fn get_type(&self) -> BoxType {
        BoxType::OpusBox
    }

    pub fn get_size(&self) -> u64 {
        let mut size = HEADER_SIZE + 8 + 20;
        size += self.dops.box_size();
        size
    }
}

impl Mp4Box for OpusBox {
    fn box_type(&self) -> BoxType {
        self.get_type()
    }

    fn box_size(&self) -> u64 {
        self.get_size()
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).unwrap())
    }

    fn summary(&self) -> Result<String> {
        let s = format!(
            "channel_count={} sample_size={} sample_rate={}",
            self.channelcount,
            self.samplesize,
            self.samplerate.value()
        );
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for OpusBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        reader.read_u32::<BigEndian>()?; // reserved
        reader.read_u16::<BigEndian>()?; // reserved
        let data_reference_index = reader.read_u16::<BigEndian>()?;
        let _version = reader.read_u16::<BigEndian>()?;
        reader.read_u16::<BigEndian>()?; // reserved
        reader.read_u32::<BigEndian>()?; // reserved
        let channelcount = reader.read_u16::<BigEndian>()?;
        let samplesize = reader.read_u16::<BigEndian>()?;
        reader.read_u32::<BigEndian>()?; // pre-defined, reserved
        let samplerate = FixedPointU16::new_raw(reader.read_u32::<BigEndian>()?);

        // read dOps box
        let header = BoxHeader::read(reader)?;
        let BoxHeader {
            name: _name,
            size: s,
        } = header;
        let dops = DopsBox::read_box(reader, s)?;

        // This shouldn't happen:
        let end = start + size;
        skip_bytes_to(reader, end)?;

        Ok(OpusBox {
            data_reference_index,
            channelcount,
            samplesize,
            samplerate,
            dops,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for OpusBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        writer.write_u32::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(self.data_reference_index)?;

        writer.write_u64::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(self.channelcount)?;
        writer.write_u16::<BigEndian>(self.samplesize)?;
        writer.write_u32::<BigEndian>(0)?; // reserved
        writer.write_u32::<BigEndian>(self.samplerate.raw_value())?;

        self.dops.write_box(writer)?;

        Ok(size)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ChannelMappingTable {
    pub stream_count: u8,
    pub coupled_count: u8,
    pub channel_mapping: Vec<u8>, // len == channel_count
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct DopsBox {
    pub version: u8,
    pub channel_count: u8,
    pub pre_skip: u16,
    // Input sample rate (32 bits unsigned, little endian): informational only
    pub sample_rate: u32,
    // Output gain (16 bits, little endian, signed Q7.8 in dB) to apply when decoding
    pub output_gain: i16,
    // Channel mapping family (8 bits unsigned)
    // -  0 = one stream: mono or L,R stereo
    // -  1 = channels in vorbis spec order: mono or L,R stereo or ... or FL,C,FR,RL,RR,LFE, ...
    // -  2..254 = reserved (treat as 255)
    // -  255 = no defined channel meaning
    pub channel_mapping_family: u8,
    // The ChannelMapping field shall be set to the same octet string as
    // *Channel Mapping* field in the identification header defined in Ogg Opus
    pub channel_mapping_table: Option<ChannelMappingTable>,
}

impl DopsBox {
    pub fn new(config: &OpusConfig) -> Self {
        Self {
            version: 0,
            channel_count: config.chan_conf as u8,
            pre_skip: config.pre_skip,
            sample_rate: config.freq_index.freq(),
            output_gain: 0,
            channel_mapping_family: 0,
            channel_mapping_table: None,
        }
    }
}

impl Mp4Box for DopsBox {
    fn box_type(&self) -> BoxType {
        BoxType::DopsBox
    }

    fn box_size(&self) -> u64 {
        HEADER_SIZE + 11 // TODO add channel mapping table size
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).unwrap())
    }

    fn summary(&self) -> Result<String> {
        Ok(String::new())
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for DopsBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;
        let end = start + size;

        let version = reader.read_u8()?;
        let channel_count = reader.read_u8()?;
        let pre_skip = reader.read_u16::<BigEndian>()?;
        let sample_rate = reader.read_u32::<BigEndian>()?;
        let output_gain = reader.read_i16::<BigEndian>()?;
        let channel_mapping_family = reader.read_u8()?;

        // TODO parse channel_mapping_table.
        skip_bytes_to(reader, end)?;

        Ok(DopsBox {
            channel_count,
            version,
            pre_skip,
            sample_rate,
            output_gain,
            channel_mapping_family,
            channel_mapping_table: None,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for DopsBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        writer.write_u8(self.version)?;
        writer.write_u8(self.channel_count)?;
        writer.write_u16::<BigEndian>(self.pre_skip)?;
        writer.write_u32::<BigEndian>(self.sample_rate)?;

        writer.write_i16::<BigEndian>(self.output_gain)?;
        writer.write_u8(self.channel_mapping_family)?;

        // TODO write channel_mapping_table

        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_opus() {
        let src_box = OpusBox {
            data_reference_index: 1,
            channelcount: 2,
            samplesize: 16,
            samplerate: FixedPointU16::new(48000),
            dops: DopsBox {
                version: 0,
                channel_count: 2,
                pre_skip: 0,
                sample_rate: 48000,
                output_gain: 0,
                channel_mapping_family: 0,
                channel_mapping_table: None,
            },
        };
        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::OpusBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = OpusBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
