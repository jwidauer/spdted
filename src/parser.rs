use super::coordinate_2d::Coordinate2d;
use super::tile::{DtedHeader, DtedTile};

use ndarray::{Array2, ShapeBuilder};
use nom::{
    bytes::complete::{tag, take},
    character::complete::{one_of, u16},
    combinator::verify,
    number::complete::be_u32,
    IResult, Parser,
};
use std::io;
use std::mem::MaybeUninit;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("io error: {0}")]
    Io(io::Error),
    #[error("parse error: {0}")]
    Invalid(String),
}

// Parse an angle in the format DDDMMSSH
#[inline(always)]
fn parse_angle(input: &[u8]) -> IResult<&[u8], f64> {
    let (input, degrees) = take(3u8).and_then(u16).parse(input)?;
    // According to the spec, the MMSS part is always 0, so we ignore it
    let (input, _) = take(4u8)(input)?;
    let (input, hemisphere) = one_of("NESW")(input)?;
    let sign = match hemisphere {
        'N' | 'E' => 1.0,
        'S' | 'W' => -1.0,
        _ => unreachable!(),
    };
    let angle = sign * degrees as f64;
    Ok((input, angle))
}

#[inline(always)]
fn parse_user_header_label(input: &[u8]) -> IResult<&[u8], DtedHeader> {
    let (input, _) = tag("UHL1")(input)?;
    let (input, origin_lon) =
        verify(parse_angle, |lon| (-180.0..180.0).contains(lon)).parse(input)?;
    let (input, origin_lat) =
        verify(parse_angle, |lat| (-90.0..90.0).contains(lat)).parse(input)?;
    // We ignore the next 5 fields (lon interval [4], lat interval [4], accuracy [4],
    // security code [3] and unique ref [12]) -> 27 bytes
    let (input, _) = take(27u8)(input)?;
    let (input, num_lon_points) = take(4u8).and_then(u16).map(|x| x as usize).parse(input)?;
    let (input, num_lat_points) = take(4u8).and_then(u16).map(|x| x as usize).parse(input)?;
    // We ignore the next 4 fields (multiple accuracy [1], reserved [24]) -> 25 bytes
    let (input, _) = take(25u8)(input)?;

    let origin = Coordinate2d::from_degrees(origin_lat, origin_lon)
        .expect("this should not fail because we already checked the bounds");
    let header = DtedHeader {
        origin_sw: origin,
        num_lat_points,
        num_lon_points,
    };

    Ok((input, header))
}

// Convert two big endian signed magnitude bytes to a two's complement 16 bit integer
#[inline(always)]
fn parse_height(input: &[u8]) -> i16 {
    let x = ((input[0] as u16) << 8) | input[1] as u16;
    let x = x as i16;
    let mask = x >> 15;
    (!mask & x) | (mask & ((x & (1 << 15)) - x))
}

#[inline(always)]
fn parse_dted_record_into<'a>(
    input: &'a [u8],
    buf: &mut [MaybeUninit<i16>],
) -> Result<(), nom::Err<nom::error::Error<&'a [u8]>>> {
    const HEADER_SIZE: usize = 8;
    const CHECKSUM_SIZE: usize = 4;
    let record_size = buf.len() * 2 + HEADER_SIZE;
    let total_record_size = record_size + CHECKSUM_SIZE;

    let expected_checksum = input[..record_size]
        .iter()
        .fold(0u32, |acc, &x| acc + x as u32);

    let checksum = be_u32(&input[record_size..total_record_size]).map(|(_, checksum)| checksum)?;
    if expected_checksum != checksum {
        return Err(nom::Err::Error(nom::error::make_error(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let input = &input[HEADER_SIZE..record_size];
    for (elem, bytes) in buf.iter_mut().zip(input.chunks_exact(2)) {
        elem.write(parse_height(bytes));
    }

    Ok(())
}

#[inline(always)]
fn parse_dted_data<'a>(header: &DtedHeader, input: &'a [u8]) -> IResult<&'a [u8], Array2<i16>> {
    let n_lats = header.num_lat();
    let n_lons = header.num_lon();
    let n_elevations = n_lats * n_lons;

    const RECORD_HEADER_SIZE: usize = 8;
    const RECORD_CHECKSUM_SIZE: usize = 4;
    let record_size = n_lats * 2 + RECORD_HEADER_SIZE + RECORD_CHECKSUM_SIZE;
    let required_input_size = record_size * n_lons;

    if input.len() < required_input_size {
        return Err(nom::Err::Error(nom::error::make_error(
            input,
            nom::error::ErrorKind::Eof,
        )));
    }

    let mut data: Vec<i16> = Vec::with_capacity(n_elevations);
    for (col, input) in data
        .spare_capacity_mut()
        .chunks_exact_mut(n_lats)
        .zip(input.chunks_exact(record_size))
    {
        // TODO: check what happens if this fails
        parse_dted_record_into(input, col)?;
    }

    // SAFETY: we just wrote `n_elevations` elements into the vector
    unsafe {
        data.set_len(n_elevations);
    }

    let data = Array2::from_shape_vec((n_lats, n_lons).f(), data).unwrap();

    let input = &input[record_size * n_lons..];

    Ok((input, data))
}

#[inline(always)]
pub fn parse_dted_tile(input: &[u8]) -> IResult<&[u8], DtedTile> {
    let (input, header) = parse_user_header_label(input)?;
    // Skip DSI [648] and ACC [2700] fields -> 3348 bytes
    let (input, _) = take(3348u16)(input)?;
    let (input, data) = parse_dted_data(&header, input)?;
    Ok((input, DtedTile { header, data }))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_angle() {
        let input = b"0000000N";
        let expected = Ok((&[][..], 0.0));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"0000000S";
        let expected = Ok((&[][..], 0.0));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"0010000N";
        let expected = Ok((&[][..], 1.0));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"0010000S";
        let expected = Ok((&[][..], -1.0));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"0900000N";
        let expected = Ok((&[][..], 90.0));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"0900000S";
        let expected = Ok((&[][..], -90.0));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"1800000N";
        let expected = Ok((&[][..], 180.0));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"1800000S";
        let expected = Ok((&[][..], -180.0));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"2700000N";
        let expected = Ok((&[][..], 270.0));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"2700000W";
        let expected = Ok((&[][..], -270.0));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"3600000N";
        let expected = Ok((&[][..], 360.0));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"3600000W";
        let expected = Ok((&[][..], -360.0));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"0010000E";
        let expected = Ok((&[][..], 1.0));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"abcdefgh";
        let expected = Err(nom::Err::Error(nom::error::Error::new(
            &input[..3],
            nom::error::ErrorKind::Digit,
        )));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"0000000X";
        let expected = Err(nom::Err::Error(nom::error::Error::new(
            &input[7..],
            nom::error::ErrorKind::OneOf,
        )));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"0000000";
        let expected = Err(nom::Err::Error(nom::error::Error::new(
            &[] as &[u8],
            nom::error::ErrorKind::OneOf,
        )));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"0";
        let expected = Err(nom::Err::Error(nom::error::Error::new(
            &input[..],
            nom::error::ErrorKind::Eof,
        )));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);

        let input = b"0123";
        let expected = Err(nom::Err::Error(nom::error::Error::new(
            &input[3..],
            nom::error::ErrorKind::Eof,
        )));
        let actual = parse_angle(input);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_height_parser() {
        let x = [0b00000000, 0b00000000];
        assert_eq!(super::parse_height(&x), 0);

        let x = [0b00000000, 0b00000001];
        assert_eq!(super::parse_height(&x), 1);

        let x = [0b00000000, 0b00000010];
        assert_eq!(super::parse_height(&x), 2);

        let x = [0b00100000, 0b00000000];
        assert_eq!(super::parse_height(&x), 8192);

        let x = [0b01000000, 0b00000000];
        assert_eq!(super::parse_height(&x), 16384);

        let x = [0b10000000, 0b00000000];
        assert_eq!(super::parse_height(&x), 0);

        let x = [0b10000000, 0b00000001];
        assert_eq!(super::parse_height(&x), -1);

        let x = [0b10000000, 0b00000010];
        assert_eq!(super::parse_height(&x), -2);

        let x = [0b10100000, 0b00000000];
        assert_eq!(super::parse_height(&x), -8192);

        let x = [0b11000000, 0b00000000];
        assert_eq!(super::parse_height(&x), -16384);
    }
}
