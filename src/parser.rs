use nom::{be_u8,be_u32,IResult,Needed};

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

#[allow(non_uppercase_globals)]
#[cfg(test)]
mod tests {
  use super::*;
  use nom::IResult;

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
    // starts at 99 bytes (header) + 4 (size of previous tag)
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
}
