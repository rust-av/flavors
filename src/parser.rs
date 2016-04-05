use nom::{be_u8,be_u16,be_i16,be_u32,be_f64,IResult,Needed,Err,ErrorKind};
use std::str::from_utf8;

/// Recognizes big endian unsigned 4 bytes integer
#[inline]
pub fn be_u24(i: &[u8]) -> IResult<&[u8], u32> {
  if i.len() < 3 {
    IResult::Incomplete(Needed::Size(3))
  } else {
    let res = ((i[0] as u32) << 16) + ((i[1] as u32) << 8) + i[2] as u32;
    IResult::Done(&i[3..], res)
  }
}

#[derive(Debug,PartialEq,Eq)]
pub struct Header {
  version: u8,
  audio:   bool,
  video:   bool,
  offset:  u32,
}

named!(pub header<Header>,
  chain!(
             tag!("FLV") ~
    version: be_u8       ~
    flags:   be_u8       ~
    offset:  be_u32      ,
    || {
      Header {
        version: version,
        audio:   flags & 4 == 4,
        video:   flags & 1 == 1,
        offset:  offset
      }
    }
  )
);

#[derive(Debug,PartialEq,Eq)]
pub enum TagType {
  Audio,
  Video,
  Script,
}

#[derive(Debug,PartialEq,Eq)]
pub struct TagHeader {
  tag_type:  TagType,
  data_size: u32,
  timestamp: u32,
  stream_id: u32,
}

#[derive(Debug,PartialEq,Eq)]
pub enum TagData {
  //Audio(AudioData),
  Audio,
  Video,
  Script,
}

#[derive(Debug,PartialEq,Eq)]
pub struct Tag {
  header: TagHeader,
  data: TagData,
}

named!(pub tag_header<TagHeader>,
  chain!(
    tag_type: switch!(be_u8,
      8  => value!(TagType::Audio) |
      9  => value!(TagType::Video) |
      18 => value!(TagType::Script)
    )                                ~
    data_size:          be_u24       ~
    timestamp:          be_u24       ~
    timestamp_extended: be_u8        ~
    stream_id:          be_u24       ,
    || {
      TagHeader {
        tag_type:  tag_type,
        data_size: data_size,
        timestamp: (timestamp_extended as u32) << 24 + timestamp,
        stream_id: stream_id,
      }
    }
  )
);

#[derive(Debug,PartialEq,Eq)]
pub enum SoundFormat {
  PCM_BE,
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

#[derive(Debug,PartialEq,Eq)]
pub enum SoundRate {
  _5_5KHZ,
  _11KHZ,
  _22KHZ,
  _44KHZ,
}

#[derive(Debug,PartialEq,Eq)]
pub enum SoundSize {
  Snd8bit,
  Snd16bit,
}

#[derive(Debug,PartialEq,Eq)]
pub enum SoundType {
  SndMono,
  SndStereo,
}

#[derive(Debug,PartialEq,Eq)]
pub struct AudioData<'a> {
  sound_format: SoundFormat,
  sound_rate:   SoundRate,
  sound_size:   SoundSize,
  sound_type:   SoundType,
  sound_data:   &'a [u8]
}

pub fn audio_data(input: &[u8], size: usize) -> IResult<&[u8], AudioData> {
  if input.len() < size {
    return IResult::Incomplete(Needed::Size(size));
  }

  let (remaining, (sformat, srate, ssize, stype)) = try_parse!(input, bits!(
    tuple!(
      switch!(take_bits!(u8, 4),
        0  => value!(SoundFormat::PCM_BE)
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
      switch!(take_bits!(u8, 2),
        0 => value!(SoundRate::_5_5KHZ)
      | 1 => value!(SoundRate::_11KHZ)
      | 2 => value!(SoundRate::_22KHZ)
      | 3 => value!(SoundRate::_44KHZ)
      ),
      switch!(take_bits!(u8, 1),
        0 => value!(SoundSize::Snd8bit)
      | 1 => value!(SoundSize::Snd16bit)
      ),
      switch!(take_bits!(u8, 1),
        0 => value!(SoundType::SndMono)
      | 1 => value!(SoundType::SndStereo)
      )
    )
  ));

  IResult::Done(&input[size..], AudioData {
    sound_format: sformat,
    sound_rate:   srate,
    sound_size:   ssize,
    sound_type:   stype,
    sound_data:   &input[1..size]
  })
}

#[derive(Debug,PartialEq,Eq)]
pub enum FrameType {
  Key,
  Inter,
  DisposableInter,
  Generated,
  Command,
}

#[derive(Debug,PartialEq,Eq)]
pub enum CodecId {
  JPEG,
  H263,
  SCREEN,
  VP6,
  VP6A,
  SCREEN2,
  H264,
}

#[derive(Debug,PartialEq,Eq)]
pub struct VideoData<'a> {
  frame_type: FrameType,
  codec_id:   CodecId,
  video_data: &'a [u8]
}

pub fn video_data(input: &[u8], size: usize) -> IResult<&[u8], VideoData> {
  if input.len() < size {
    return IResult::Incomplete(Needed::Size(size));
  }

  let (remaining, (frame_type, codec_id)) = try_parse!(input, bits!(
    tuple!(
      switch!(take_bits!(u8, 4),
        1  => value!(FrameType::Key)
      | 2  => value!(FrameType::Inter)
      | 3  => value!(FrameType::DisposableInter)
      | 4  => value!(FrameType::Generated)
      | 5  => value!(FrameType::Command)
      ),
      switch!(take_bits!(u8, 4),
        1 => value!(CodecId::JPEG)
      | 2 => value!(CodecId::H263)
      | 3 => value!(CodecId::SCREEN)
      | 4 => value!(CodecId::VP6)
      | 5 => value!(CodecId::VP6A)
      | 6 => value!(CodecId::SCREEN2)
      | 7 => value!(CodecId::H264)
      )
    )
  ));

  IResult::Done(&input[size..], VideoData {
    frame_type: frame_type,
    codec_id:   codec_id,
    video_data: &input[1..size]
  })
}

#[derive(Debug,PartialEq,Eq)]
pub struct ScriptDataObject<'a> {
  name: &'a str,
  data: ScriptDataValue<'a>,
}

#[derive(Debug,PartialEq,Eq)]
pub struct ScriptDataDate {
  date_time: f64,
  local_date_time_offset: i16, // SI16
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScriptDataValue<'a> {
  Number(f64),
  Boolean(bool),
  String(&'a str),
  Object(Vec<ScriptDataObject<'a>>),
  MovieClip(&'a str),
  Null,
  UNdefined,
  Reference(u16),
  ECMAArray(Vec<ScriptDataObject<'a>>),
  StrictArray(Vec<ScriptDataObject<'a>>),
  Date(ScriptDataDate),
  LongString(&'a str),
}

trait OwnEq {
  fn assert_receiver_is_total_eq(&self) {}
}

impl OwnEq for f64 {}

named!(pub script_data_object<ScriptDataObject>,
  chain!(
    name: script_data_string ~
    data: get_data_value,
    || {
      ScriptDataObject {
        name: name,
        data: data,
      }
    }
  )
);

pub fn script_data_object_end(input:&[u8]) -> IResult<&[u8],bool> {
  if input.len() < 1 {
    return IResult::Done(input,true)
  } else {
    match be_u24(input) {
      IResult::Done(i,o) => {
        IResult::Done(i,o == 9)
        //IResult::Error(Err::Code(ErrorKind::Tag))
      },
      IResult::Incomplete(i) => IResult::Incomplete(i),
      IResult::Error(i) => IResult::Error(i),
    }
  }
}

named!(pub script_data_string<&str>, map_res!(length_bytes!(be_u16), from_utf8));
named!(pub script_data_long_string<&str>, map_res!(length_bytes!(be_u32), from_utf8));
named!(pub script_data_date<ScriptDataDate>,
  chain!(
    date_time: be_f64 ~
    local_date_time_offset: be_i16,
    || {
      ScriptDataDate {
        date_time: date_time,
        local_date_time_offset: local_date_time_offset,
      }
    }
  )
);
named!(pub script_data_objects<Vec<ScriptDataObject> >,
  terminated!(many0!(chain!(
    name: script_data_string ~
    value: get_data_value,
    || {
      ScriptDataObject {
        name: name,
        data: value,
      }
    })), script_data_object_end)
  //terminated!(many0!(pair!(script_data_string, script_data_object)), script_data_object_end)
);
named!(pub script_data_ECMA_array<Vec<ScriptDataObject> >,
  chain!(
    be_u32 ~
    v: script_data_objects,
    || { v }
  )
);
pub fn script_data_strict_array(input: &[u8]) -> IResult<&[u8], Vec<ScriptDataObject>> {
  match be_u32(input) {
    IResult::Done(i, o) => many_m_n!(i, 1, o as usize, script_data_object),
    IResult::Incomplete(i) => IResult::Incomplete(i),
    IResult::Error(i) => IResult::Error(i),
  }
}

named!(pub get_data_value<ScriptDataValue>,
  switch!(be_u8,
      0  => map!(be_f64, |n| ScriptDataValue::Number(n))
    | 1  => map!(be_u8, |n| ScriptDataValue::Boolean(n != 0))
    | 2  => map!(script_data_string, |n| ScriptDataValue::String(n))
    | 3  => map!(script_data_objects, |n| ScriptDataValue::Object(n))
    | 4  => map!(script_data_string, |n| ScriptDataValue::MovieClip(n))
    | 5  => value!(ScriptDataValue::Null) // to remove
    | 6  => value!(ScriptDataValue::UNdefined) // to remove
    | 7  => map!(be_u16, |n| ScriptDataValue::Reference(n))
    | 8  => map!(script_data_ECMA_array, |n| ScriptDataValue::ECMAArray(n))
    | 10 => map!(script_data_strict_array, |n| ScriptDataValue::StrictArray(n))
    | 11 => map!(script_data_date, |n| ScriptDataValue::Date(n))
    | 12 => map!(script_data_long_string, |n| ScriptDataValue::LongString(n))
  )
);

// 9 is the end marker of Object type
/*named!(pub script_data_value<Vec<ScriptDataValue> >,
  terminated!(many0!(switch!(be_u8,
      0  => map!(be_f64, |n| ScriptDataValue::Number(n))
    | 1  => map!(be_u8, |n| ScriptDataValue::Boolean(n != 0))
    | 2  => map!(script_data_string, |n| ScriptDataValue::String(n))
    | 3  => map!(script_data_objects, |n| ScriptDataValue::Object(n))
    | 4  => map!(script_data_string, |n| ScriptDataValue::MovieClip(n))
    | 5  => value!(ScriptDataValue::Null) // to remove
    | 6  => value!(ScriptDataValue::UNdefined) // to remove
    | 7  => map!(be_u16, |n| ScriptDataValue::Reference(n))
    | 8  => map!(script_data_ECMA_array, |n| ScriptDataValue::ECMAArray(n))
    | 10 => map!(script_data_strict_array, |n| ScriptDataValue::StrictArray(n))
    | 11 => map!(script_data_date, |n| ScriptDataValue::Date(n))
    | 12 => map!(script_data_long_string, |n| ScriptDataValue::LongString(n))
  )), script_data_object_end)
);*/

#[derive(Debug,PartialEq,Eq)]
pub struct ScriptData<'a> {
  objects: Vec<ScriptDataObject<'a>>,
}

#[allow(non_uppercase_globals)]
#[cfg(test)]
mod tests {
  use super::*;
  use nom::{IResult,be_u16,be_u32,HexDisplay};

  const zelda       : &'static [u8] = include_bytes!("../assets/zelda.flv");
  const zeldaHQ     : &'static [u8] = include_bytes!("../assets/zeldaHQ.flv");
  const commercials : &'static [u8] = include_bytes!("../assets/asian-commercials-are-weird.flv");

  #[test]
  fn headers() {
    assert_eq!(
      header(&zelda[..9]),
      IResult::Done(
        &b""[..],
        Header { version: 1, audio: true, video: true, offset: 9 }
    ));
    assert_eq!(
      header(&zeldaHQ[..9]),
      IResult::Done(
        &b""[..],
        Header { version: 1, audio: true, video: true, offset: 9 }
    ));
    assert_eq!(
      header(&commercials[..9]),
      IResult::Done(
        &b""[..],
        Header { version: 1, audio: true, video: true, offset: 9 }
    ));
  }

  #[test]
  fn first_tag_headers() {
    // starts at 9 bytes (header) + 4 (size of previous tag)
    // header is 11 bytes long
    assert_eq!(
      tag_header(&zelda[13..24]),
      IResult::Done(
        &b""[..],
        TagHeader { tag_type: TagType::Video, data_size: 537, timestamp: 0, stream_id: 0 }
    ));
    assert_eq!(
      tag_header(&zeldaHQ[13..24]),
      IResult::Done(
        &b""[..],
        TagHeader { tag_type: TagType::Video, data_size: 2984, timestamp: 0, stream_id: 0 }
    ));
    assert_eq!(
      tag_header(&commercials[13..24]),
      IResult::Done(
        &b""[..],
        TagHeader { tag_type: TagType::Script, data_size: 273, timestamp: 0, stream_id: 0 }
    ));
  }

  #[test]
  fn audio_tags() {
    let tag_start = 24+537+4;
    println!("size of previous tag: {:?}", be_u32(&zelda[24+537..tag_start]));
    assert_eq!(
      tag_header(&zelda[tag_start..tag_start+11]),
      IResult::Done(
        &b""[..],
        TagHeader { tag_type: TagType::Audio, data_size: 642, timestamp: 0, stream_id: 0 }
    ));

    let tag_start2 = 24+2984+4;
    println!("size of previous tag: {:?}", be_u32(&zeldaHQ[24+2984..tag_start2]));
    println!("data:\n{}", (&zeldaHQ[tag_start2..tag_start2+11]).to_hex(8));
    assert_eq!(
      tag_header(&zeldaHQ[tag_start2..tag_start2+11]),
      IResult::Done(
        &b""[..],
        TagHeader { tag_type: TagType::Audio, data_size: 642, timestamp: 0, stream_id: 0 }
    ));


    println!("data: {:?}", audio_data(&zelda[tag_start+11..tag_start+11+642], 642));
    println!("data: {:?}", audio_data(&zeldaHQ[tag_start2+11..tag_start2+11+642], 642));
    assert_eq!(
      audio_data(&zelda[tag_start+11..tag_start+11+642], 642),
      IResult::Done(
        &b""[..],
        AudioData {
          sound_format: SoundFormat::ADPCM,
          sound_rate:   SoundRate::_22KHZ,
          sound_size:   SoundSize::Snd16bit,
          sound_type:   SoundType::SndMono,
          sound_data:   &zelda[tag_start+12..tag_start+11+642]
        }
    ));

    assert_eq!(
      audio_data(&zeldaHQ[tag_start2+11..tag_start2+11+642], 642),
      IResult::Done(
        &b""[..],
        AudioData {
          sound_format: SoundFormat::ADPCM,
          sound_rate:   SoundRate::_22KHZ,
          sound_size:   SoundSize::Snd16bit,
          sound_type:   SoundType::SndMono,
          sound_data:   &zeldaHQ[tag_start2+12..tag_start2+11+642]
        }
    ));
  }

  #[test]
  fn video_tags() {
    let tag_start = 24;
    assert_eq!(
      video_data(&zelda[tag_start..tag_start+537], 537),
      IResult::Done(
        &b""[..],
        VideoData {
          frame_type: FrameType::Key,
          codec_id:   CodecId::H263,
          video_data: &zelda[tag_start+1..tag_start+537]
        }
    ));
    assert_eq!(
      video_data(&zelda[tag_start..tag_start+2984], 2984),
      IResult::Done(
        &b""[..],
        VideoData {
          frame_type: FrameType::Key,
          codec_id:   CodecId::H263,
          video_data: &zelda[tag_start+1..tag_start+2984]
        }
    ));
  }

  #[test]
  fn script_tags() {
    let tag_start = 24;
    println!("--> {:?}", &zelda[tag_start+537..tag_start+539]);
    match be_u16(&zelda[tag_start+537..]) {
      IResult::Done(_,y) => {
        println!("--> {:?}", y);
      }
      _ => {}
    };
    match script_data_objects(&zelda[tag_start+537..]) {
      IResult::Done(x,y) => {
        assert_eq!(
          (&x[..20], y) as (&[u8], Vec<ScriptDataObject>),
          (&b""[..], vec!(ScriptDataObject { name: "", data: ScriptDataValue::Null })));
      }
      _ => assert!(false),
    }
    /*assert_eq!(
      script_data_value(&zelda[tag_start+2984..]),
      IResult::Done(
        &b""[..],
        ScriptDataValue::Null
    ));*/
  }

}
