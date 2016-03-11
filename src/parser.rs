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

#[cfg(test)]
mod tests {
  use super::*;
  use nom::IResult;

  const zelda       : &'static [u8] = include_bytes!("../assets/zelda.flv");
  const zeldaHQ     : &'static [u8] = include_bytes!("../assets/zeldaHQ.flv");
  const commercials : &'static [u8] = include_bytes!("../assets/asian-commercials-are-weird.flv");

  #[test]
  fn headers() {
    println!("zelda: {:?}", header(&zelda[..9]));
    println!("zeldaHQ: {:?}", header(&zeldaHQ[..9]));
    println!("commercials: {:?}", header(&commercials[..9]));
    assert!(false);
  }
}
