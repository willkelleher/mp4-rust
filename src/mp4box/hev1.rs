use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Seek, Write};
use serde::{Serialize};

use crate::mp4box::*;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hev1Box {
    pub data_reference_index: u16,
    pub width: u16,
    pub height: u16,

    #[serde(with = "value_u32")]
    pub horizresolution: FixedPointU16,

    #[serde(with = "value_u32")]
    pub vertresolution: FixedPointU16,
    pub frame_count: u16,
    pub depth: u16,
    pub hvcc: HvcCBox,
}

impl Default for Hev1Box {
    fn default() -> Self {
        Hev1Box {
            data_reference_index: 0,
            width: 0,
            height: 0,
            horizresolution: FixedPointU16::new(0x48),
            vertresolution: FixedPointU16::new(0x48),
            frame_count: 1,
            depth: 0x0018,
            hvcc: HvcCBox::default(),
        }
    }
}

impl Hev1Box {
    pub fn new(config: &HevcConfig) -> Self {
        Hev1Box {
            data_reference_index: 1,
            width: config.width,
            height: config.height,
            horizresolution: FixedPointU16::new(0x48),
            vertresolution: FixedPointU16::new(0x48),
            frame_count: 1,
            depth: 0x0018,
            hvcc: HvcCBox::new(&config.seq_param_set, &config.pic_param_set, &config.vid_param_set),
        }
    }

    pub fn get_type(&self) -> BoxType {
        BoxType::Hev1Box
    }

    pub fn get_size(&self) -> u64 {
        HEADER_SIZE + 8 + 70 + self.hvcc.box_size()
    }
}

impl Mp4Box for Hev1Box {
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
        let s = format!("data_reference_index={} width={} height={} frame_count={}",
            self.data_reference_index, self.width, self.height, self.frame_count);
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for Hev1Box {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        reader.read_u32::<BigEndian>()?; // reserved
        reader.read_u16::<BigEndian>()?; // reserved
        let data_reference_index = reader.read_u16::<BigEndian>()?;

        reader.read_u32::<BigEndian>()?; // pre-defined, reserved
        reader.read_u64::<BigEndian>()?; // pre-defined
        reader.read_u32::<BigEndian>()?; // pre-defined
        let width = reader.read_u16::<BigEndian>()?;
        let height = reader.read_u16::<BigEndian>()?;
        let horizresolution = FixedPointU16::new_raw(reader.read_u32::<BigEndian>()?);
        let vertresolution = FixedPointU16::new_raw(reader.read_u32::<BigEndian>()?);
        reader.read_u32::<BigEndian>()?; // reserved
        let frame_count = reader.read_u16::<BigEndian>()?;
        skip_bytes(reader, 32)?; // compressorname
        let depth = reader.read_u16::<BigEndian>()?;
        reader.read_i16::<BigEndian>()?; // pre-defined

        let header = BoxHeader::read(reader)?;
        let BoxHeader { name, size: s } = header;
        if name == BoxType::HvcCBox {
            let hvcc = HvcCBox::read_box(reader, s)?;

            skip_bytes_to(reader, start + size)?;

            Ok(Hev1Box {
                data_reference_index,
                width,
                height,
                horizresolution,
                vertresolution,
                frame_count,
                depth,
                hvcc,
            })
        } else {
            Err(Error::InvalidData("hvcc not found"))
        }
    }
}

impl<W: Write> WriteBox<&mut W> for Hev1Box {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        writer.write_u32::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(self.data_reference_index)?;

        writer.write_u32::<BigEndian>(0)?; // pre-defined, reserved
        writer.write_u64::<BigEndian>(0)?; // pre-defined
        writer.write_u32::<BigEndian>(0)?; // pre-defined
        writer.write_u16::<BigEndian>(self.width)?;
        writer.write_u16::<BigEndian>(self.height)?;
        writer.write_u32::<BigEndian>(self.horizresolution.raw_value())?;
        writer.write_u32::<BigEndian>(self.vertresolution.raw_value())?;
        writer.write_u32::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(self.frame_count)?;
        // skip compressorname
        write_zeros(writer, 32)?;
        writer.write_u16::<BigEndian>(self.depth)?;
        writer.write_i16::<BigEndian>(-1)?; // pre-defined

        self.hvcc.write_box(writer)?;

        Ok(size)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct HvcCBox {
    pub configuration_version: u8,
    pub sequence_parameter_sets: Vec<NalUnit>,
    pub picture_parameter_sets: Vec<NalUnit>,
    pub video_parameter_sets: Vec<NalUnit>,
}

impl HvcCBox {
    pub fn new(sps: &[u8], pps: &[u8], vps: &[u8]) -> Self {
        Self {
            configuration_version: 1,
            sequence_parameter_sets: vec![NalUnit::from(sps)],
            picture_parameter_sets: vec![NalUnit::from(pps)],
            video_parameter_sets: vec![NalUnit::from(vps)],
        }
    }
}

impl Mp4Box for HvcCBox {
    fn box_type(&self) -> BoxType {
        BoxType::HvcCBox
    }

    fn box_size(&self) -> u64 {
        let mut size = HEADER_SIZE + 32;
        for vps in self.video_parameter_sets.iter() {
            size += vps.size() as u64;
        }
        for sps in self.sequence_parameter_sets.iter() {
            size += sps.size() as u64;
        }
        for pps in self.picture_parameter_sets.iter() {
            size += pps.size() as u64;
        }
        size
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).unwrap())
    }

    fn summary(&self) -> Result<String> {
        let s = format!("configuration_version={}",
            self.configuration_version);
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for HvcCBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let configuration_version = reader.read_u8()?;
        let _ = reader.read_u8()?; // TODO
        let _ = reader.read_u32::<BigEndian>()?;
        let _ = reader.read_u48::<BigEndian>()?;
        let _ = reader.read_u8()?;
        let _ = reader.read_u16::<BigEndian>()?;
        let _ = reader.read_u8()?;
        let _ = reader.read_u8()?;
        let _ = reader.read_u8()?; // bitDepthLumaMinus8
        let _ = reader.read_u8()?; // bitDepthChromaMinus8
        let _ = reader.read_u16::<BigEndian>()?;
        let _ = reader.read_u8()?;
        let _num_arrays = reader.read_u8()?; // numArrays

        let num_of_vpss = reader.read_u8()?;
        let mut video_parameter_sets = Vec::with_capacity(num_of_vpss as usize);
        for _ in 0..num_of_vpss {
            let nal_unit = NalUnit::read(reader)?;
            video_parameter_sets.push(nal_unit);
        }
        let num_of_spss = reader.read_u8()? & 0x1F;
        let mut sequence_parameter_sets = Vec::with_capacity(num_of_spss as usize);
        for _ in 0..num_of_spss {
            let nal_unit = NalUnit::read(reader)?;
            sequence_parameter_sets.push(nal_unit);
        }
        let num_of_ppss = reader.read_u8()?;
        let mut picture_parameter_sets = Vec::with_capacity(num_of_ppss as usize);
        for _ in 0..num_of_ppss {
            let nal_unit = NalUnit::read(reader)?;
            picture_parameter_sets.push(nal_unit);
        }

        skip_bytes_to(reader, start + size)?;

        Ok(HvcCBox {
            configuration_version,
            video_parameter_sets,
            sequence_parameter_sets,
            picture_parameter_sets,
        })
    }
}

const VPS_NAL_TYPE: u8 = 32;
const SPS_NAL_TYPE: u8 = 33;
const PPS_NAL_TYPE: u8 = 34;

impl<W: Write> WriteBox<&mut W> for HvcCBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        writer.write_u8(self.configuration_version)?;
        writer.write_u8(0)?; // general_profile_space, general_tier_flag, general_profile_idc
        writer.write_u32::<BigEndian>(0)?; // general_profile_compatibility_flags
        writer.write_u48::<BigEndian>(0)?; // general_constraint_indicator_flags
        writer.write_u8(0)?; // general_level_idc
        writer.write_u16::<BigEndian>(0xf000)?; // min_spatial_segmentation_idc
        writer.write_u8(0xfc | 0)?; // parallelismType
        writer.write_u8(0xfc | 0)?; // chromaFormat
        writer.write_u8(2 | 0xf8)?; // bitDepthLumaMinus8
        writer.write_u8(2 | 0xf8)?; // bitDepthChromaMinus8
        writer.write_u16::<BigEndian>(0)?; // avgFrameRate
        writer.write_u8(0 << 6 | 1 << 3 | 1 << 2 | 3)?; //constantFrameRate, numTemporarlLayers, temporalIdNested, lengthSizeMinusOne
        writer.write_u8(3)?; // numArrays

        // here we write NAL arrays, one for each of our three basic required
        // types (VPS, SPS, PPS) with a fixed length of 1 per array. obviously
        // this is not very generic.

        let array_completeness = 1;

        writer.write_u8(array_completeness << 7 | VPS_NAL_TYPE & 0x3f)?;
        writer.write_u16::<BigEndian>(self.video_parameter_sets.len() as u16)?;
        for sps in self.video_parameter_sets.iter() {
            sps.write(writer)?;
        }

        writer.write_u8(array_completeness << 7 | SPS_NAL_TYPE & 0x3f)?;
        writer.write_u16::<BigEndian>(self.sequence_parameter_sets.len() as u16)?;
        for sps in self.sequence_parameter_sets.iter() {
            sps.write(writer)?;
        }

        writer.write_u8(array_completeness << 7 | PPS_NAL_TYPE & 0x3f)?;
        writer.write_u16::<BigEndian>(self.picture_parameter_sets.len() as u16)?;
        for pps in self.picture_parameter_sets.iter() {
            pps.write(writer)?;
        }

        Ok(size)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct NalUnit {
    pub bytes: Vec<u8>,
}

impl From<&[u8]> for NalUnit {
    fn from(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.to_vec(),
        }
    }
}

impl NalUnit {
    fn size(&self) -> usize {
        2 + self.bytes.len()
    }

    fn read<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let length = reader.read_u16::<BigEndian>()? as usize;
        let mut bytes = vec![0u8; length];
        reader.read(&mut bytes)?;
        Ok(NalUnit { bytes })
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<u64> {
        writer.write_u16::<BigEndian>(self.bytes.len() as u16)?;
        writer.write(&self.bytes)?;
        Ok(self.size() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_hev1() {
        let src_box = Hev1Box {
            data_reference_index: 1,
            width: 320,
            height: 240,
            horizresolution: FixedPointU16::new(0x48),
            vertresolution: FixedPointU16::new(0x48),
            frame_count: 1,
            depth: 24,
            hvcc: HvcCBox {
                configuration_version: 1,
            },
        };
        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::Hev1Box);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = Hev1Box::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
