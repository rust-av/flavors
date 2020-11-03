use nom::{IResult,Err,Needed};
use nom::number::streaming::{be_u8,be_u16,be_i16,be_u24,be_i24,be_u32,be_f64};
use std::str::from_utf8;

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Header {
  pub version: u8,
  pub audio:   bool,
  pub video:   bool,
  pub offset:  u32,
}

named!(pub header<Header>,
  do_parse!(
             tag!("FLV") >>
    version: be_u8       >>
    flags:   be_u8       >>
    offset:  be_u32      >>
    (Header {
        version: version,
        audio:   flags & 4 == 4,
        video:   flags & 1 == 1,
        offset:  offset
    })
  )
);

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum TagType {
  Audio,
  Video,
  Script,
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct TagHeader {
  pub tag_type:  TagType,
  pub data_size: u32,
  pub timestamp: u32,
  pub stream_id: u32,
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum TagData<'a> {
  //Audio(AudioData),
  Audio(AudioData<'a>),
  Video(VideoData<'a>),
  Script,
}

#[derive(Debug,PartialEq,Eq)]
pub struct Tag<'a> {
  pub header: TagHeader,
  pub data:   TagData<'a>,
}

named!(pub tag_header<TagHeader>,
  do_parse!(
    tag_type: switch!(be_u8,
      8  => value!(TagType::Audio) |
      9  => value!(TagType::Video) |
      18 => value!(TagType::Script)
    )                                >>
    data_size:          be_u24       >>
    timestamp:          be_u24       >>
    timestamp_extended: be_u8        >>
    stream_id:          be_u24       >>
    (TagHeader {
        tag_type:  tag_type,
        data_size: data_size,
        timestamp: ((timestamp_extended as u32) << 24) + timestamp,
        stream_id: stream_id,
    })
  )
);

macro_rules! tag_data(
  ($i:expr, $tag_type:expr, $size:expr) => (
    tag_data($tag_type, $size)($i)
  );
);

named!(pub complete_tag<Tag>,
  do_parse!(
    tag_type: switch!(be_u8,
      8  => value!(TagType::Audio) |
      9  => value!(TagType::Video) |
      18 => value!(TagType::Script)
    )                                >>
    data_size:          be_u24       >>
    timestamp:          be_u24       >>
    timestamp_extended: be_u8        >>
    stream_id:          be_u24       >>
    data: tag_data!(tag_type, data_size as usize) >>
    (Tag {
      header: TagHeader {
        tag_type:  tag_type,
        data_size: data_size,
        timestamp: ((timestamp_extended as u32) << 24) + timestamp,
        stream_id: stream_id,
      },
      data: data
    })
  )
);

pub fn tag_data(tag_type: TagType, size: usize) -> impl Fn(&[u8]) -> IResult<&[u8], TagData> {
  move |input: &[u8]| {
      match tag_type {
        TagType::Video  => map!(input, |i| video_data(i, size), |v| TagData::Video(v)),
        TagType::Audio  => map!(input, |i| audio_data(i, size), |v| TagData::Audio(v)),
        TagType::Script => value!(input, TagData::Script)
      }
  }
}


#[allow(non_camel_case_types)]
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
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

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SoundRate {
  _5_5KHZ,
  _11KHZ,
  _22KHZ,
  _44KHZ,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SoundSize {
  Snd8bit,
  Snd16bit,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SoundType {
  SndMono,
  SndStereo,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum AACPacketType {
  SequenceHeader,
  Raw,
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct AACAudioPacketHeader {
  pub packet_type: AACPacketType,
}

named!(pub aac_audio_packet_header<AACAudioPacketHeader>,
  do_parse!(
    packet_type: switch!(be_u8,
      0  => value!(AACPacketType::SequenceHeader) |
      1  => value!(AACPacketType::Raw)
    )                                >>
    (AACAudioPacketHeader {
        packet_type: packet_type,
    })
  )
);

#[derive(Debug,PartialEq,Eq)]
pub struct AACAudioPacket<'a> {
  pub packet_type: AACPacketType,
  pub aac_data:    &'a [u8]
}

pub fn aac_audio_packet(input: &[u8], size: usize) -> IResult<&[u8], AACAudioPacket> {
  if input.len() < size {
    return Err(Err::Incomplete(Needed::new(size)));
  }

  if size < 1 {
    return Err(Err::Incomplete(Needed::new(1)));
  }

  let (_remaining, packet_type) = try_parse!(input, switch!(be_u8,
      0  => value!(AACPacketType::SequenceHeader) |
      1  => value!(AACPacketType::Raw)
    )
  );

  Ok((&input[size..], AACAudioPacket {
    packet_type: packet_type,
    aac_data:    &input[1..size]
  }))
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct AudioData<'a> {
  pub sound_format: SoundFormat,
  pub sound_rate:   SoundRate,
  pub sound_size:   SoundSize,
  pub sound_type:   SoundType,
  pub sound_data:   &'a [u8]
}

pub fn audio_data(input: &[u8], size: usize) -> IResult<&[u8], AudioData> {
  if input.len() < size {
    return Err(Err::Incomplete(Needed::new(size)));
  }

  if size < 1 {
    return Err(Err::Incomplete(Needed::new(1)));
  }

  let (_remaining, (sformat, srate, ssize, stype)) = try_parse!(input, bits!(
    tuple!(
      switch!(take_bits!(4u8),
        0  => value!(SoundFormat::PCM_NE)
      | 1  => value!(SoundFormat::ADPCM)
      | 2  => value!(SoundFormat::MP3)
      | 3  => value!(SoundFormat::PCM_LE)
      | 4  => value!(SoundFormat::NELLYMOSER_16KHZ_MONO)
      | 5  => value!(SoundFormat::NELLYMOSER_8KHZ_MONO)
      | 6  => value!(SoundFormat::NELLYMOSER)
      | 7  => value!(SoundFormat::PCM_ALAW)
      | 8  => value!(SoundFormat::PCM_ULAW)
      | 10 => value!(SoundFormat::AAC)
      | 11 => value!(SoundFormat::SPEEX)
      | 14 => value!(SoundFormat::MP3_8KHZ)
      | 15 => value!(SoundFormat::DEVICE_SPECIFIC)
      ),
      switch!(take_bits!(2u8),
        0 => value!(SoundRate::_5_5KHZ)
      | 1 => value!(SoundRate::_11KHZ)
      | 2 => value!(SoundRate::_22KHZ)
      | 3 => value!(SoundRate::_44KHZ)
      ),
      switch!(take_bits!(1u8),
        0 => value!(SoundSize::Snd8bit)
      | 1 => value!(SoundSize::Snd16bit)
      ),
      switch!(take_bits!(1u8),
        0 => value!(SoundType::SndMono)
      | 1 => value!(SoundType::SndStereo)
      )
    )
  ));

  Ok((&input[size..], AudioData {
    sound_format: sformat,
    sound_rate:   srate,
    sound_size:   ssize,
    sound_type:   stype,
    sound_data:   &input[1..size]
  }))
}

#[derive(Debug,PartialEq,Eq)]
pub struct AudioDataHeader {
  pub sound_format: SoundFormat,
  pub sound_rate:   SoundRate,
  pub sound_size:   SoundSize,
  pub sound_type:   SoundType,
}

pub fn audio_data_header(input: &[u8]) -> IResult<&[u8], AudioDataHeader> {
  if input.len() < 1 {
    return Err(Err::Incomplete(Needed::new(1)));
  }

  let (remaining, (sformat, srate, ssize, stype)) = try_parse!(input, bits!(
    tuple!(
      switch!(take_bits!(4u8),
        0  => value!(SoundFormat::PCM_NE)
      | 1  => value!(SoundFormat::ADPCM)
      | 2  => value!(SoundFormat::MP3)
      | 3  => value!(SoundFormat::PCM_LE)
      | 4  => value!(SoundFormat::NELLYMOSER_16KHZ_MONO)
      | 5  => value!(SoundFormat::NELLYMOSER_8KHZ_MONO)
      | 6  => value!(SoundFormat::NELLYMOSER)
      | 7  => value!(SoundFormat::PCM_ALAW)
      | 8  => value!(SoundFormat::PCM_ULAW)
      | 10 => value!(SoundFormat::AAC)
      | 11 => value!(SoundFormat::SPEEX)
      | 14 => value!(SoundFormat::MP3_8KHZ)
      | 15 => value!(SoundFormat::DEVICE_SPECIFIC)
      ),
      switch!(take_bits!(2u8),
        0 => value!(SoundRate::_5_5KHZ)
      | 1 => value!(SoundRate::_11KHZ)
      | 2 => value!(SoundRate::_22KHZ)
      | 3 => value!(SoundRate::_44KHZ)
      ),
      switch!(take_bits!(1u8),
        0 => value!(SoundSize::Snd8bit)
      | 1 => value!(SoundSize::Snd16bit)
      ),
      switch!(take_bits!(1u8),
        0 => value!(SoundType::SndMono)
      | 1 => value!(SoundType::SndStereo)
      )
    )
  ));

  Ok((remaining, AudioDataHeader {
    sound_format: sformat,
    sound_rate:   srate,
    sound_size:   ssize,
    sound_type:   stype,
  }))
}


#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum FrameType {
  Key,
  Inter,
  DisposableInter,
  Generated,
  Command,
}

#[allow(non_camel_case_types)]
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
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

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum AVCPacketType {
  SequenceHeader,
  NALU,
  EndOfSequence,
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct AVCVideoPacketHeader {
  pub packet_type:      AVCPacketType,
  pub composition_time: i32,
}

named!(pub avc_video_packet_header<AVCVideoPacketHeader>,
  do_parse!(
    packet_type: switch!(be_u8,
      0  => value!(AVCPacketType::SequenceHeader) |
      1  => value!(AVCPacketType::NALU) |
      2  => value!(AVCPacketType::EndOfSequence)
    )                                >>
    composition_time:   be_i24       >>
    (AVCVideoPacketHeader {
        packet_type:      packet_type,
        composition_time: composition_time,
    })
  )
);

#[derive(Debug,PartialEq,Eq)]
pub struct AVCVideoPacket<'a> {
  pub packet_type:      AVCPacketType,
  pub composition_time: i32,
  pub avc_data:         &'a [u8]
}

pub fn avc_video_packet(input: &[u8], size: usize) -> IResult<&[u8], AVCVideoPacket> {
  if input.len() < size {
    return Err(Err::Incomplete(Needed::new(size)));
  }

  if size < 4 {
    return Err(Err::Incomplete(Needed::new(4)));
  }

  let (_remaining, (packet_type, composition_time)) = try_parse!(input, tuple!(
    switch!(be_u8,
      0  => value!(AVCPacketType::SequenceHeader) |
      1  => value!(AVCPacketType::NALU) |
      2  => value!(AVCPacketType::EndOfSequence)
    ),
    be_i24
  ));

  Ok((&input[size..], AVCVideoPacket {
    packet_type:      packet_type,
    composition_time: composition_time,
    avc_data:         &input[4..size]
  }))
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct VideoData<'a> {
  pub frame_type: FrameType,
  pub codec_id:   CodecId,
  pub video_data: &'a [u8]
}

pub fn video_data(input: &[u8], size: usize) -> IResult<&[u8], VideoData> {
  if input.len() < size {
    return Err(Err::Incomplete(Needed::new(size)));
  }

  if size < 1 {
    return Err(Err::Incomplete(Needed::new(1)));
  }

  let (_remaining, (frame_type, codec_id)) = try_parse!(input, bits!(
    tuple!(
      switch!(take_bits!(4u8),
        1  => value!(FrameType::Key)
      | 2  => value!(FrameType::Inter)
      | 3  => value!(FrameType::DisposableInter)
      | 4  => value!(FrameType::Generated)
      | 5  => value!(FrameType::Command)
      ),
      switch!(take_bits!(4u8),
        1 => value!(CodecId::JPEG)
      | 2 => value!(CodecId::SORENSON_H263)
      | 3 => value!(CodecId::SCREEN)
      | 4 => value!(CodecId::VP6)
      | 5 => value!(CodecId::VP6A)
      | 6 => value!(CodecId::SCREEN2)
      | 7 => value!(CodecId::H264)
      | 8 => value!(CodecId::H263)
      | 9 => value!(CodecId::MPEG4Part2)
      )
    )
  ));

  Ok((&input[size..], VideoData {
    frame_type: frame_type,
    codec_id:   codec_id,
    video_data: &input[1..size]
  }))
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct VideoDataHeader {
  pub frame_type: FrameType,
  pub codec_id:   CodecId,
}

pub fn video_data_header(input: &[u8]) -> IResult<&[u8], VideoDataHeader> {
  if input.len() < 1 {
    return Err(Err::Incomplete(Needed::new(1)));
  }

  let (remaining, (frame_type, codec_id)) = try_parse!(input, bits!(
    tuple!(
      switch!(take_bits!(4u8),
        1  => value!(FrameType::Key)
      | 2  => value!(FrameType::Inter)
      | 3  => value!(FrameType::DisposableInter)
      | 4  => value!(FrameType::Generated)
      | 5  => value!(FrameType::Command)
      ),
      switch!(take_bits!(4u8),
        1 => value!(CodecId::JPEG)
      | 2 => value!(CodecId::SORENSON_H263)
      | 3 => value!(CodecId::SCREEN)
      | 4 => value!(CodecId::VP6)
      | 5 => value!(CodecId::VP6A)
      | 6 => value!(CodecId::SCREEN2)
      | 7 => value!(CodecId::H264)
      | 8 => value!(CodecId::H263)
      | 9 => value!(CodecId::MPEG4Part2)
      )
    )
  ));

  Ok((remaining, VideoDataHeader {
    frame_type: frame_type,
    codec_id:   codec_id,
  }))
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

#[derive(Clone,Debug, PartialEq)]
pub struct ScriptDataDate {
  pub date_time: f64,
  pub local_date_time_offset: i16, // SI16
}

#[allow(non_upper_case_globals)]
static script_data_name_tag: &'static [u8] = &[2];
named!(pub script_data<ScriptData>,
  do_parse!(
    // Must start with a string, i.e. 2
    tag!(script_data_name_tag)   >>
    name: script_data_string     >>
    arguments: script_data_value >>
    (ScriptData {
        name: name,
        arguments: arguments,
    })
    )
);

named!(pub script_data_value<ScriptDataValue>,
  switch!(be_u8,
      0  => map!(be_f64, |n| ScriptDataValue::Number(n))
    | 1  => map!(be_u8, |n| ScriptDataValue::Boolean(n != 0))
    | 2  => map!(script_data_string, |n| ScriptDataValue::String(n))
    | 3  => map!(script_data_objects, |n| ScriptDataValue::Object(n))
    | 4  => map!(script_data_string, |n| ScriptDataValue::MovieClip(n))
    | 5  => value!(ScriptDataValue::Null) // to remove
    | 6  => value!(ScriptDataValue::Undefined) // to remove
    | 7  => map!(be_u16, |n| ScriptDataValue::Reference(n))
    | 8  => map!(script_data_ECMA_array, |n| ScriptDataValue::ECMAArray(n))
    | 10 => map!(script_data_strict_array, |n| ScriptDataValue::StrictArray(n))
    | 11 => map!(script_data_date, |n| ScriptDataValue::Date(n))
    | 12 => map!(script_data_long_string, |n| ScriptDataValue::LongString(n))
  )
);

named!(pub script_data_objects<Vec<ScriptDataObject> >,
  terminated!(many0!(do_parse!(
    name: script_data_string >>
    value: script_data_value >>
    (ScriptDataObject {
        name: name,
        data: value,
    })
    )), script_data_object_end)
);

named!(pub script_data_object<ScriptDataObject>,
  do_parse!(
    name: script_data_string >>
    data: script_data_value  >>
    (ScriptDataObject {
        name: name,
        data: data,
    })
  )
);

#[allow(non_upper_case_globals)]
static script_data_object_end_terminator: &'static [u8] = &[0, 0, 9];
pub fn script_data_object_end(input:&[u8]) -> IResult<&[u8],&[u8]> {
  tag!(input, script_data_object_end_terminator)
}

named!(pub script_data_string<&str>, map_res!(length_data!(be_u16), from_utf8));
named!(pub script_data_long_string<&str>, map_res!(length_data!(be_u32), from_utf8));
named!(pub script_data_date<ScriptDataDate>,
  do_parse!(
    date_time: be_f64               >>
    local_date_time_offset: be_i16  >>
    (ScriptDataDate {
        date_time: date_time,
        local_date_time_offset: local_date_time_offset,
    })
  )
);

named!(pub script_data_ECMA_array<Vec<ScriptDataObject> >,
  do_parse!(
    be_u32                 >>
    v: script_data_objects >>
    (v)
  )
);

pub fn script_data_strict_array(input: &[u8]) -> IResult<&[u8], Vec<ScriptDataValue>> {
  match be_u32(input) {
    Ok((i, o)) => many_m_n!(i, 1, o as usize, script_data_value),
    Err(err) => Err(err),
  }
}

#[allow(non_upper_case_globals)]
#[cfg(test)]
mod tests {
  use super::*;
  use nom::HexDisplay;
  use nom::number::streaming::be_u32;

  const zelda       : &'static [u8] = include_bytes!("../assets/zelda.flv");
  const zeldaHQ     : &'static [u8] = include_bytes!("../assets/zeldaHQ.flv");
  const commercials : &'static [u8] = include_bytes!("../assets/asian-commercials-are-weird.flv");

  #[test]
  fn headers() {
    assert_eq!(
      header(&zelda[..9]),
      Ok((
        &b""[..],
        Header { version: 1, audio: true, video: true, offset: 9 }
    )));
    assert_eq!(
      header(&zeldaHQ[..9]),
      Ok((
        &b""[..],
        Header { version: 1, audio: true, video: true, offset: 9 }
    )));
    assert_eq!(
      header(&commercials[..9]),
      Ok((
        &b""[..],
        Header { version: 1, audio: true, video: true, offset: 9 }
    )));
  }

  #[test]
  fn first_tag_headers() {
    // starts at 9 bytes (header) + 4 (size of previous tag)
    // header is 11 bytes long
    assert_eq!(
      tag_header(&zelda[13..24]),
      Ok((
        &b""[..],
        TagHeader { tag_type: TagType::Video, data_size: 537, timestamp: 0, stream_id: 0 }
    )));
    assert_eq!(
      tag_header(&zeldaHQ[13..24]),
      Ok((
        &b""[..],
        TagHeader { tag_type: TagType::Video, data_size: 2984, timestamp: 0, stream_id: 0 }
    )));
    assert_eq!(
      tag_header(&commercials[13..24]),
      Ok((
        &b""[..],
        TagHeader { tag_type: TagType::Script, data_size: 273, timestamp: 0, stream_id: 0 }
    )));
  }

  #[test]
  fn audio_tags() {
    let tag_start = 24+537+4;
    println!("size of previous tag: {:?}", be_u32::<_, ()>(&zelda[24+537..tag_start]));
    assert_eq!(
      tag_header(&zelda[tag_start..tag_start+11]),
      Ok((
        &b""[..],
        TagHeader { tag_type: TagType::Audio, data_size: 642, timestamp: 0, stream_id: 0 }
    )));

    let tag_start2 = 24+2984+4;
    println!("size of previous tag: {:?}", be_u32::<_, ()>(&zeldaHQ[24+2984..tag_start2]));
    println!("data:\n{}", (&zeldaHQ[tag_start2..tag_start2+11]).to_hex(8));
    assert_eq!(
      tag_header(&zeldaHQ[tag_start2..tag_start2+11]),
      Ok((
        &b""[..],
        TagHeader { tag_type: TagType::Audio, data_size: 642, timestamp: 0, stream_id: 0 }
    )));


    println!("data: {:?}", audio_data(&zelda[tag_start+11..tag_start+11+642], 642));
    println!("data: {:?}", audio_data(&zeldaHQ[tag_start2+11..tag_start2+11+642], 642));
    assert_eq!(
      audio_data(&zelda[tag_start+11..tag_start+11+642], 642),
      Ok((
        &b""[..],
        AudioData {
          sound_format: SoundFormat::ADPCM,
          sound_rate:   SoundRate::_22KHZ,
          sound_size:   SoundSize::Snd16bit,
          sound_type:   SoundType::SndMono,
          sound_data:   &zelda[tag_start+12..tag_start+11+642]
        }
    )));

    assert_eq!(
      audio_data(&zeldaHQ[tag_start2+11..tag_start2+11+642], 642),
      Ok((
        &b""[..],
        AudioData {
          sound_format: SoundFormat::ADPCM,
          sound_rate:   SoundRate::_22KHZ,
          sound_size:   SoundSize::Snd16bit,
          sound_type:   SoundType::SndMono,
          sound_data:   &zeldaHQ[tag_start2+12..tag_start2+11+642]
        }
    )));
  }

  #[test]
  fn video_tags() {
    let tag_start = 24;
    assert_eq!(
      video_data(&zelda[tag_start..tag_start+537], 537),
      Ok((
        &b""[..],
        VideoData {
          frame_type: FrameType::Key,
          codec_id:   CodecId::SORENSON_H263,
          video_data: &zelda[tag_start+1..tag_start+537]
        }
    )));
    assert_eq!(
      video_data(&zeldaHQ[tag_start..tag_start+2984], 2984),
      Ok((
        &b""[..],
        VideoData {
          frame_type: FrameType::Key,
          codec_id:   CodecId::SORENSON_H263,
          video_data: &zeldaHQ[tag_start+1..tag_start+2984]
        }
    )));
  }

  #[test]
  fn script_tags() {
    let tag_start = 24;
    let tag_end = tag_start + 273;

    match script_data(&commercials[tag_start..tag_end]) {
      Ok((remaining,script_data)) => {
        assert_eq!(remaining.len(), 0);
        assert_eq!(script_data,
          ScriptData {
            name: "onMetaData",
            arguments: ScriptDataValue::ECMAArray(
              vec![
                ScriptDataObject {
                  name: "duration", data: ScriptDataValue::Number(28.133)
                },
                ScriptDataObject {
                  name: "width", data: ScriptDataValue::Number(464.0)
                },
                ScriptDataObject {
                  name: "height", data: ScriptDataValue::Number(348.0)
                },
                ScriptDataObject {
                  name: "videodatarate", data: ScriptDataValue::Number(368.0)
                },
                ScriptDataObject {
                  name: "framerate", data: ScriptDataValue::Number(30.0)
                },
                ScriptDataObject {
                  name: "videocodecid", data: ScriptDataValue::Number(4.0)
                },
                ScriptDataObject {
                  name: "audiodatarate", data: ScriptDataValue::Number(56.0)
                },
                ScriptDataObject {
                  name: "audiodelay", data: ScriptDataValue::Number(0.0)
                },
                ScriptDataObject {
                  name: "audiocodecid", data: ScriptDataValue::Number(2.0)
                },
                ScriptDataObject {
                  name: "canSeekToEnd", data: ScriptDataValue::Number(1.0)
                },
                ScriptDataObject {
                  name: "creationdate", data: ScriptDataValue::String("Thu Oct 04 18:37:42 2007\n")
                }
              ]
            )
          }
        );
      }
      _ => unreachable!(),
    }
  }

  #[test]
  fn complete_video_tags() {
    let tag_start      = 13;
    let tag_data_start = tag_start + 11;
    assert_eq!(
      complete_tag(&zelda[tag_start..tag_data_start+537]),
      Ok((
        &b""[..],
        Tag {
          header: TagHeader { tag_type: TagType::Video, data_size: 537, timestamp: 0, stream_id: 0 },
          data: TagData::Video(VideoData {
            frame_type: FrameType::Key,
            codec_id:   CodecId::SORENSON_H263,
            video_data: &zelda[tag_data_start+1..tag_data_start+537]
          })
        }
      ))
    );
    assert_eq!(
      complete_tag(&zeldaHQ[tag_start..tag_data_start+2984]),
      Ok((
        &b""[..],
        Tag {
          header: TagHeader { tag_type: TagType::Video, data_size: 2984, timestamp: 0, stream_id: 0 },
          data: TagData::Video(VideoData {
            frame_type: FrameType::Key,
            codec_id:   CodecId::SORENSON_H263,
            video_data: &zeldaHQ[tag_data_start+1..tag_data_start+2984]
          })
        }
      ))
    );
  }

}
