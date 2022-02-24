use std::str::from_utf8;

use nom::bits::bits;
use nom::bits::streaming::take;
use nom::bytes::streaming::tag;
use nom::combinator::{flat_map, map, map_res};
use nom::error::{Error, ErrorKind};
use nom::multi::{length_data, many0, many_m_n};
use nom::number::streaming::{be_f64, be_i16, be_i24, be_u16, be_u24, be_u32, be_u8};
use nom::sequence::{pair, terminated, tuple};
use nom::{Err, IResult, Needed};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
  pub version: u8,
  pub audio: bool,
  pub video: bool,
  pub offset: u32,
}

pub fn header(input: &[u8]) -> IResult<&[u8], Header> {
  tuple((tag("FLV"), be_u8, be_u8, be_u32))(input).map(|(i, (_, version, flags, offset))| {
    (
      i,
      Header {
        version,
        audio: flags & 4 == 4,
        video: flags & 1 == 1,
        offset,
      },
    )
  })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagType {
  Audio,
  Video,
  Script,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagHeader {
  pub tag_type: TagType,
  pub data_size: u32,
  pub timestamp: u32,
  pub stream_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagData<'a> {
  Audio(AudioData<'a>),
  Video(VideoData<'a>),
  Script,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Tag<'a> {
  pub header: TagHeader,
  pub data: TagData<'a>,
}
fn tag_type(input: &[u8]) -> IResult<&[u8], TagType> {
  be_u8(input).and_then(|(i, tag_type)| {
    Ok((
      i,
      match tag_type {
        8 => TagType::Audio,
        9 => TagType::Video,
        18 => TagType::Script,
        _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
      },
    ))
  })
}

pub fn tag_header(input: &[u8]) -> IResult<&[u8], TagHeader> {
  tuple((tag_type, be_u24, be_u24, be_u8, be_u24))(input).map(
    |(i, (tag_type, data_size, timestamp, timestamp_extended, stream_id))| {
      (
        i,
        TagHeader {
          tag_type,
          data_size,
          timestamp: (u32::from(timestamp_extended) << 24) + timestamp,
          stream_id,
        },
      )
    },
  )
}

pub fn complete_tag(input: &[u8]) -> IResult<&[u8], Tag> {
  pair(tag_type, be_u24)(input).and_then(|(i, (tag_type, data_size))| {
    tuple((
      be_u24,
      be_u8,
      be_u24,
      tag_data(tag_type, data_size as usize),
    ))(i)
    .map(|(i, (timestamp, timestamp_extended, stream_id, data))| {
      (
        i,
        Tag {
          header: TagHeader {
            tag_type,
            data_size,
            timestamp: (u32::from(timestamp_extended) << 24) + timestamp,
            stream_id,
          },
          data,
        },
      )
    })
  })
}

pub fn tag_data(tag_type: TagType, size: usize) -> impl Fn(&[u8]) -> IResult<&[u8], TagData> {
  move |input| match tag_type {
    TagType::Video => map(|i| video_data(i, size), TagData::Video)(input),
    TagType::Audio => map(|i| audio_data(i, size), TagData::Audio)(input),
    TagType::Script => Ok((input, TagData::Script)),
  }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundFormat {
  PCM_NE, // native endianness...
  ADPCM,
  MP3,
  PCM_LE,
  NELLYMOSER_16KHZ_MONO,
  NELLYMOSER_8KHZ_MONO,
  NELLYMOSER,
  PCM_ALAW,
  PCM_ULAW,
  AAC,
  SPEEX,
  MP3_8KHZ,
  DEVICE_SPECIFIC,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundRate {
  _5_5KHZ,
  _11KHZ,
  _22KHZ,
  _44KHZ,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundSize {
  Snd8bit,
  Snd16bit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundType {
  SndMono,
  SndStereo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AACPacketType {
  SequenceHeader,
  Raw,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AACAudioPacketHeader {
  pub packet_type: AACPacketType,
}

pub fn aac_audio_packet_header(input: &[u8]) -> IResult<&[u8], AACAudioPacketHeader> {
  be_u8(input).and_then(|(i, packet_type)| {
    Ok((
      i,
      AACAudioPacketHeader {
        packet_type: match packet_type {
          0 => AACPacketType::SequenceHeader,
          1 => AACPacketType::Raw,
          _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
        },
      },
    ))
  })
}

#[derive(Debug, PartialEq, Eq)]
pub struct AACAudioPacket<'a> {
  pub packet_type: AACPacketType,
  pub aac_data: &'a [u8],
}

pub fn aac_audio_packet(input: &[u8], size: usize) -> IResult<&[u8], AACAudioPacket> {
  if input.len() < size {
    return Err(Err::Incomplete(Needed::new(size)));
  }

  if size < 1 {
    return Err(Err::Incomplete(Needed::new(1)));
  }

  be_u8(input).and_then(|(_, packet_type)| {
    Ok((
      &input[size..],
      AACAudioPacket {
        packet_type: match packet_type {
          0 => AACPacketType::SequenceHeader,
          1 => AACPacketType::Raw,
          _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
        },
        aac_data: &input[1..size],
      },
    ))
  })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioData<'a> {
  pub sound_format: SoundFormat,
  pub sound_rate: SoundRate,
  pub sound_size: SoundSize,
  pub sound_type: SoundType,
  pub sound_data: &'a [u8],
}

pub fn audio_data(input: &[u8], size: usize) -> IResult<&[u8], AudioData> {
  if input.len() < size {
    return Err(Err::Incomplete(Needed::new(size)));
  }

  if size < 1 {
    return Err(Err::Incomplete(Needed::new(1)));
  }

  let take_bits = tuple((take(4usize), take(2usize), take(1usize), take(1usize)));
  bits::<_, _, Error<_>, _, _>(take_bits)(input).and_then(|(_, (sformat, srate, ssize, stype))| {
    let sformat = match sformat {
      0 => SoundFormat::PCM_NE,
      1 => SoundFormat::ADPCM,
      2 => SoundFormat::MP3,
      3 => SoundFormat::PCM_LE,
      4 => SoundFormat::NELLYMOSER_16KHZ_MONO,
      5 => SoundFormat::NELLYMOSER_8KHZ_MONO,
      6 => SoundFormat::NELLYMOSER,
      7 => SoundFormat::PCM_ALAW,
      8 => SoundFormat::PCM_ULAW,
      10 => SoundFormat::AAC,
      11 => SoundFormat::SPEEX,
      14 => SoundFormat::MP3_8KHZ,
      15 => SoundFormat::DEVICE_SPECIFIC,
      _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
    };
    let srate = match srate {
      0 => SoundRate::_5_5KHZ,
      1 => SoundRate::_11KHZ,
      2 => SoundRate::_22KHZ,
      3 => SoundRate::_44KHZ,
      _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
    };
    let ssize = match ssize {
      0 => SoundSize::Snd8bit,
      1 => SoundSize::Snd16bit,
      _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
    };
    let stype = match stype {
      0 => SoundType::SndMono,
      1 => SoundType::SndStereo,
      _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
    };

    Ok((
      &input[size..],
      AudioData {
        sound_format: sformat,
        sound_rate: srate,
        sound_size: ssize,
        sound_type: stype,
        sound_data: &input[1..size],
      },
    ))
  })
}

#[derive(Debug, PartialEq, Eq)]
pub struct AudioDataHeader {
  pub sound_format: SoundFormat,
  pub sound_rate: SoundRate,
  pub sound_size: SoundSize,
  pub sound_type: SoundType,
}

pub fn audio_data_header(input: &[u8]) -> IResult<&[u8], AudioDataHeader> {
  if input.is_empty() {
    return Err(Err::Incomplete(Needed::new(1)));
  }

  let take_bits = tuple((take(4usize), take(2usize), take(1usize), take(1usize)));
  bits::<_, _, Error<_>, _, _>(take_bits)(input).and_then(
    |(remaining, (sformat, srate, ssize, stype))| {
      let sformat = match sformat {
        0 => SoundFormat::PCM_NE,
        1 => SoundFormat::ADPCM,
        2 => SoundFormat::MP3,
        3 => SoundFormat::PCM_LE,
        4 => SoundFormat::NELLYMOSER_16KHZ_MONO,
        5 => SoundFormat::NELLYMOSER_8KHZ_MONO,
        6 => SoundFormat::NELLYMOSER,
        7 => SoundFormat::PCM_ALAW,
        8 => SoundFormat::PCM_ULAW,
        10 => SoundFormat::AAC,
        11 => SoundFormat::SPEEX,
        14 => SoundFormat::MP3_8KHZ,
        15 => SoundFormat::DEVICE_SPECIFIC,
        _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
      };
      let srate = match srate {
        0 => SoundRate::_5_5KHZ,
        1 => SoundRate::_11KHZ,
        2 => SoundRate::_22KHZ,
        3 => SoundRate::_44KHZ,
        _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
      };
      let ssize = match ssize {
        0 => SoundSize::Snd8bit,
        1 => SoundSize::Snd16bit,
        _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
      };
      let stype = match stype {
        0 => SoundType::SndMono,
        1 => SoundType::SndStereo,
        _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
      };

      Ok((
        remaining,
        AudioDataHeader {
          sound_format: sformat,
          sound_rate: srate,
          sound_size: ssize,
          sound_type: stype,
        },
      ))
    },
  )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameType {
  Key,
  Inter,
  DisposableInter,
  Generated,
  Command,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecId {
  JPEG,
  SORENSON_H263,
  SCREEN,
  VP6,
  VP6A,
  SCREEN2,
  H264,
  // Not in FLV standard
  H263,
  MPEG4Part2, // MPEG-4 Part 2
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AVCPacketType {
  SequenceHeader,
  NALU,
  EndOfSequence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AVCVideoPacketHeader {
  pub packet_type: AVCPacketType,
  pub composition_time: i32,
}

fn packet_type(input: &[u8]) -> IResult<&[u8], AVCPacketType> {
  be_u8(input).and_then(|(i, packet_type)| {
    Ok((
      i,
      match packet_type {
        0 => AVCPacketType::SequenceHeader,
        1 => AVCPacketType::NALU,
        2 => AVCPacketType::EndOfSequence,
        _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
      },
    ))
  })
}

pub fn avc_video_packet_header(input: &[u8]) -> IResult<&[u8], AVCVideoPacketHeader> {
  pair(packet_type, be_i24)(input).map(|(i, (packet_type, composition_time))| {
    (
      i,
      AVCVideoPacketHeader {
        packet_type,
        composition_time,
      },
    )
  })
}

#[derive(Debug, PartialEq, Eq)]
pub struct AVCVideoPacket<'a> {
  pub packet_type: AVCPacketType,
  pub composition_time: i32,
  pub avc_data: &'a [u8],
}

pub fn avc_video_packet(input: &[u8], size: usize) -> IResult<&[u8], AVCVideoPacket> {
  if input.len() < size {
    return Err(Err::Incomplete(Needed::new(size)));
  }

  if size < 4 {
    return Err(Err::Incomplete(Needed::new(4)));
  }
  pair(packet_type, be_i24)(input).map(|(_, (packet_type, composition_time))| {
    (
      &input[size..],
      AVCVideoPacket {
        packet_type,
        composition_time,
        avc_data: &input[4..size],
      },
    )
  })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoData<'a> {
  pub frame_type: FrameType,
  pub codec_id: CodecId,
  pub video_data: &'a [u8],
}

pub fn video_data(input: &[u8], size: usize) -> IResult<&[u8], VideoData> {
  if input.len() < size {
    return Err(Err::Incomplete(Needed::new(size)));
  }

  if size < 1 {
    return Err(Err::Incomplete(Needed::new(1)));
  }

  let take_bits = pair(take(4usize), take(4usize));
  bits::<_, _, Error<_>, _, _>(take_bits)(input).and_then(|(_, (frame_type, codec_id))| {
    let frame_type = match frame_type {
      1 => FrameType::Key,
      2 => FrameType::Inter,
      3 => FrameType::DisposableInter,
      4 => FrameType::Generated,
      5 => FrameType::Command,
      _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
    };
    let codec_id = match codec_id {
      1 => CodecId::JPEG,
      2 => CodecId::SORENSON_H263,
      3 => CodecId::SCREEN,
      4 => CodecId::VP6,
      5 => CodecId::VP6A,
      6 => CodecId::SCREEN2,
      7 => CodecId::H264,
      8 => CodecId::H263,
      9 => CodecId::MPEG4Part2,
      _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
    };

    Ok((
      &input[size..],
      VideoData {
        frame_type,
        codec_id,
        video_data: &input[1..size],
      },
    ))
  })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoDataHeader {
  pub frame_type: FrameType,
  pub codec_id: CodecId,
}

pub fn video_data_header(input: &[u8]) -> IResult<&[u8], VideoDataHeader> {
  if input.is_empty() {
    return Err(Err::Incomplete(Needed::new(1)));
  }

  let take_bits = pair(take(4usize), take(4usize));
  bits::<_, _, Error<_>, _, _>(take_bits)(input).and_then(|(remaining, (frame_type, codec_id))| {
    let frame_type = match frame_type {
      1 => FrameType::Key,
      2 => FrameType::Inter,
      3 => FrameType::DisposableInter,
      4 => FrameType::Generated,
      5 => FrameType::Command,
      _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
    };
    let codec_id = match codec_id {
      1 => CodecId::JPEG,
      2 => CodecId::SORENSON_H263,
      3 => CodecId::SCREEN,
      4 => CodecId::VP6,
      5 => CodecId::VP6A,
      6 => CodecId::SCREEN2,
      7 => CodecId::H264,
      8 => CodecId::H263,
      9 => CodecId::MPEG4Part2,
      _ => return Err(Err::Error(Error::new(input, ErrorKind::Alt))),
    };

    Ok((
      remaining,
      VideoDataHeader {
        frame_type,
        codec_id,
      },
    ))
  })
}

#[derive(Debug, PartialEq)]
pub struct ScriptData<'a> {
  pub name: &'a str,
  pub arguments: ScriptDataValue<'a>,
}

#[derive(Debug, PartialEq)]
pub enum ScriptDataValue<'a> {
  Number(f64),
  Boolean(bool),
  String(&'a str),
  Object(Vec<ScriptDataObject<'a>>),
  MovieClip(&'a str),
  Null,
  Undefined,
  Reference(u16),
  ECMAArray(Vec<ScriptDataObject<'a>>),
  StrictArray(Vec<ScriptDataValue<'a>>),
  Date(ScriptDataDate),
  LongString(&'a str),
}

#[derive(Debug, PartialEq)]
pub struct ScriptDataObject<'a> {
  pub name: &'a str,
  pub data: ScriptDataValue<'a>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptDataDate {
  pub date_time: f64,
  pub local_date_time_offset: i16, // SI16
}

#[allow(non_upper_case_globals)]
static script_data_name_tag: &[u8] = &[2];

pub fn script_data(input: &[u8]) -> IResult<&[u8], ScriptData> {
  // Must start with a string, i.e. 2
  tuple((
    tag(script_data_name_tag),
    script_data_string,
    script_data_value,
  ))(input)
  .map(|(i, (_, name, arguments))| (i, ScriptData { name, arguments }))
}

pub fn script_data_value(input: &[u8]) -> IResult<&[u8], ScriptDataValue> {
  be_u8(input).and_then(|v| match v {
    (i, 0) => map(be_f64, ScriptDataValue::Number)(i),
    (i, 1) => map(be_u8, |n| ScriptDataValue::Boolean(n != 0))(i),
    (i, 2) => map(script_data_string, ScriptDataValue::String)(i),
    (i, 3) => map(script_data_objects, ScriptDataValue::Object)(i),
    (i, 4) => map(script_data_string, ScriptDataValue::MovieClip)(i),
    (i, 5) => Ok((i, ScriptDataValue::Null)), // to remove
    (i, 6) => Ok((i, ScriptDataValue::Undefined)), // to remove
    (i, 7) => map(be_u16, ScriptDataValue::Reference)(i),
    (i, 8) => map(script_data_ecma_array, ScriptDataValue::ECMAArray)(i),
    (i, 10) => map(script_data_strict_array, ScriptDataValue::StrictArray)(i),
    (i, 11) => map(script_data_date, ScriptDataValue::Date)(i),
    (i, 12) => map(script_data_long_string, ScriptDataValue::LongString)(i),
    _ => Err(Err::Error(Error::new(input, ErrorKind::Alt))),
  })
}

pub fn script_data_objects(input: &[u8]) -> IResult<&[u8], Vec<ScriptDataObject>> {
  terminated(many0(script_data_object), script_data_object_end)(input)
}

pub fn script_data_object(input: &[u8]) -> IResult<&[u8], ScriptDataObject> {
  pair(script_data_string, script_data_value)(input)
    .map(|(i, (name, data))| (i, ScriptDataObject { name, data }))
}

#[allow(non_upper_case_globals)]
static script_data_object_end_terminator: &[u8] = &[0, 0, 9];

pub fn script_data_object_end(input: &[u8]) -> IResult<&[u8], &[u8]> {
  tag(script_data_object_end_terminator)(input)
}

pub fn script_data_string(input: &[u8]) -> IResult<&[u8], &str> {
  map_res(length_data(be_u16), from_utf8)(input)
}

pub fn script_data_long_string(input: &[u8]) -> IResult<&[u8], &str> {
  map_res(length_data(be_u32), from_utf8)(input)
}

pub fn script_data_date(input: &[u8]) -> IResult<&[u8], ScriptDataDate> {
  pair(be_f64, be_i16)(input).map(|(i, (date_time, local_date_time_offset))| {
    (
      i,
      ScriptDataDate {
        date_time,
        local_date_time_offset,
      },
    )
  })
}

pub fn script_data_ecma_array(input: &[u8]) -> IResult<&[u8], Vec<ScriptDataObject>> {
  pair(be_u32, script_data_objects)(input).map(|(i, (_, data_objects))| (i, data_objects))
}

pub fn script_data_strict_array(input: &[u8]) -> IResult<&[u8], Vec<ScriptDataValue>> {
  flat_map(be_u32, |o| many_m_n(1, o as usize, script_data_value))(input)
}

#[allow(non_upper_case_globals)]
#[cfg(test)]
mod tests {
  use super::*;
  use nom::number::streaming::be_u32;
  use nom::HexDisplay;

  const zelda: &[u8] = include_bytes!("../assets/zelda.flv");
  const zeldaHQ: &[u8] = include_bytes!("../assets/zeldaHQ.flv");
  const commercials: &[u8] = include_bytes!("../assets/asian-commercials-are-weird.flv");

  #[test]
  fn headers() {
    assert_eq!(
      header(&zelda[..9]),
      Ok((
        &b""[..],
        Header {
          version: 1,
          audio: true,
          video: true,
          offset: 9
        }
      ))
    );
    assert_eq!(
      header(&zeldaHQ[..9]),
      Ok((
        &b""[..],
        Header {
          version: 1,
          audio: true,
          video: true,
          offset: 9
        }
      ))
    );
    assert_eq!(
      header(&commercials[..9]),
      Ok((
        &b""[..],
        Header {
          version: 1,
          audio: true,
          video: true,
          offset: 9
        }
      ))
    );
  }

  #[test]
  fn first_tag_headers() {
    // starts at 9 bytes (header) + 4 (size of previous tag)
    // header is 11 bytes long
    assert_eq!(
      tag_header(&zelda[13..24]),
      Ok((
        &b""[..],
        TagHeader {
          tag_type: TagType::Video,
          data_size: 537,
          timestamp: 0,
          stream_id: 0
        }
      ))
    );
    assert_eq!(
      tag_header(&zeldaHQ[13..24]),
      Ok((
        &b""[..],
        TagHeader {
          tag_type: TagType::Video,
          data_size: 2984,
          timestamp: 0,
          stream_id: 0
        }
      ))
    );
    assert_eq!(
      tag_header(&commercials[13..24]),
      Ok((
        &b""[..],
        TagHeader {
          tag_type: TagType::Script,
          data_size: 273,
          timestamp: 0,
          stream_id: 0
        }
      ))
    );
  }

  #[test]
  fn audio_tags() {
    let tag_start = 24 + 537 + 4;
    println!(
      "size of previous tag: {:?}",
      be_u32::<_, ()>(&zelda[24 + 537..tag_start])
    );
    assert_eq!(
      tag_header(&zelda[tag_start..tag_start + 11]),
      Ok((
        &b""[..],
        TagHeader {
          tag_type: TagType::Audio,
          data_size: 642,
          timestamp: 0,
          stream_id: 0
        }
      ))
    );

    let tag_start2 = 24 + 2984 + 4;
    println!(
      "size of previous tag: {:?}",
      be_u32::<_, ()>(&zeldaHQ[24 + 2984..tag_start2])
    );
    println!(
      "data:\n{}",
      (&zeldaHQ[tag_start2..tag_start2 + 11]).to_hex(8)
    );
    assert_eq!(
      tag_header(&zeldaHQ[tag_start2..tag_start2 + 11]),
      Ok((
        &b""[..],
        TagHeader {
          tag_type: TagType::Audio,
          data_size: 642,
          timestamp: 0,
          stream_id: 0
        }
      ))
    );

    println!(
      "data: {:?}",
      audio_data(&zelda[tag_start + 11..tag_start + 11 + 642], 642)
    );
    println!(
      "data: {:?}",
      audio_data(&zeldaHQ[tag_start2 + 11..tag_start2 + 11 + 642], 642)
    );
    assert_eq!(
      audio_data(&zelda[tag_start + 11..tag_start + 11 + 642], 642),
      Ok((
        &b""[..],
        AudioData {
          sound_format: SoundFormat::ADPCM,
          sound_rate: SoundRate::_22KHZ,
          sound_size: SoundSize::Snd16bit,
          sound_type: SoundType::SndMono,
          sound_data: &zelda[tag_start + 12..tag_start + 11 + 642]
        }
      ))
    );

    assert_eq!(
      audio_data(&zeldaHQ[tag_start2 + 11..tag_start2 + 11 + 642], 642),
      Ok((
        &b""[..],
        AudioData {
          sound_format: SoundFormat::ADPCM,
          sound_rate: SoundRate::_22KHZ,
          sound_size: SoundSize::Snd16bit,
          sound_type: SoundType::SndMono,
          sound_data: &zeldaHQ[tag_start2 + 12..tag_start2 + 11 + 642]
        }
      ))
    );
  }

  #[test]
  fn video_tags() {
    let tag_start = 24;
    assert_eq!(
      video_data(&zelda[tag_start..tag_start + 537], 537),
      Ok((
        &b""[..],
        VideoData {
          frame_type: FrameType::Key,
          codec_id: CodecId::SORENSON_H263,
          video_data: &zelda[tag_start + 1..tag_start + 537]
        }
      ))
    );
    assert_eq!(
      video_data(&zeldaHQ[tag_start..tag_start + 2984], 2984),
      Ok((
        &b""[..],
        VideoData {
          frame_type: FrameType::Key,
          codec_id: CodecId::SORENSON_H263,
          video_data: &zeldaHQ[tag_start + 1..tag_start + 2984]
        }
      ))
    );
  }

  #[test]
  fn script_tags() {
    let tag_start = 24;
    let tag_end = tag_start + 273;

    match script_data(&commercials[tag_start..tag_end]) {
      Ok((remaining, script_data)) => {
        assert_eq!(remaining.len(), 0);
        assert_eq!(
          script_data,
          ScriptData {
            name: "onMetaData",
            arguments: ScriptDataValue::ECMAArray(vec![
              ScriptDataObject {
                name: "duration",
                data: ScriptDataValue::Number(28.133)
              },
              ScriptDataObject {
                name: "width",
                data: ScriptDataValue::Number(464.0)
              },
              ScriptDataObject {
                name: "height",
                data: ScriptDataValue::Number(348.0)
              },
              ScriptDataObject {
                name: "videodatarate",
                data: ScriptDataValue::Number(368.0)
              },
              ScriptDataObject {
                name: "framerate",
                data: ScriptDataValue::Number(30.0)
              },
              ScriptDataObject {
                name: "videocodecid",
                data: ScriptDataValue::Number(4.0)
              },
              ScriptDataObject {
                name: "audiodatarate",
                data: ScriptDataValue::Number(56.0)
              },
              ScriptDataObject {
                name: "audiodelay",
                data: ScriptDataValue::Number(0.0)
              },
              ScriptDataObject {
                name: "audiocodecid",
                data: ScriptDataValue::Number(2.0)
              },
              ScriptDataObject {
                name: "canSeekToEnd",
                data: ScriptDataValue::Number(1.0)
              },
              ScriptDataObject {
                name: "creationdate",
                data: ScriptDataValue::String("Thu Oct 04 18:37:42 2007\n")
              }
            ])
          }
        );
      }
      _ => unreachable!(),
    }
  }

  #[test]
  fn complete_video_tags() {
    let tag_start = 13;
    let tag_data_start = tag_start + 11;
    assert_eq!(
      complete_tag(&zelda[tag_start..tag_data_start + 537]),
      Ok((
        &b""[..],
        Tag {
          header: TagHeader {
            tag_type: TagType::Video,
            data_size: 537,
            timestamp: 0,
            stream_id: 0
          },
          data: TagData::Video(VideoData {
            frame_type: FrameType::Key,
            codec_id: CodecId::SORENSON_H263,
            video_data: &zelda[tag_data_start + 1..tag_data_start + 537]
          })
        }
      ))
    );
    assert_eq!(
      complete_tag(&zeldaHQ[tag_start..tag_data_start + 2984]),
      Ok((
        &b""[..],
        Tag {
          header: TagHeader {
            tag_type: TagType::Video,
            data_size: 2984,
            timestamp: 0,
            stream_id: 0
          },
          data: TagData::Video(VideoData {
            frame_type: FrameType::Key,
            codec_id: CodecId::SORENSON_H263,
            video_data: &zeldaHQ[tag_data_start + 1..tag_data_start + 2984]
          })
        }
      ))
    );
  }
}
