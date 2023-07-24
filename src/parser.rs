use super::coordinate_2d::Coordinate2d;
use super::tile::{DtedHeader, DtedTile};

use ndarray::{Array1, Array2};
use nom::{
    bytes::complete::{tag, take},
    character::complete::{one_of, u16},
    combinator::{consumed, verify},
    multi::count,
    number::complete::{be_u16, be_u32},
    IResult, Parser,
};
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("io error: {0}")]
    Io(io::Error),
    #[error("parse error: {0}")]
    Invalid(String),
}

// Parse an angle in the format DDDMMSSH
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

// Convert a signed magnitude 16 bit integer to a two's complement 16 bit integer
fn to_i16(x: u16) -> i16 {
    let mask = x as i16 >> 15;
    (!mask & x as i16) | (((x & (1 << 15)) as i16 - x as i16) & mask)
}

fn parse_dted_record(n_lats: usize, input: &[u8]) -> IResult<&[u8], Array1<i16>> {
    let header_parser = take(8u8);

    let n_bytes = 2 * n_lats;
    let height_parser = take(2u8).and_then(be_u16).map(to_i16);
    let column_parser = take(n_bytes).and_then(count(height_parser, n_lats));

    let record_parser = header_parser.and(column_parser);
    let record_parser = consumed(record_parser).map(|(data, (_, elevations)): (&[u8], _)| {
        let checksum: u32 = data.iter().fold(0u32, |acc, &x| acc + x as u32);
        (checksum, elevations)
    });

    let checksum_parser = take(4u8).and_then(be_u32);

    let total_record_parser = record_parser.and(checksum_parser);

    verify(
        total_record_parser,
        |&((expected_checksum, _), checksum)| expected_checksum == checksum,
    )
    .map(|((_, elevations), _)| Array1::from_vec(elevations))
    .parse(input)
}

fn parse_dted_data<'a>(header: &DtedHeader, input: &'a [u8]) -> IResult<&'a [u8], Array2<i16>> {
    let n_lats = header.num_lat();
    let n_lons = header.num_lon();

    let mut input = input;

    let mut data = Array2::default((n_lats, n_lons));
    for mut col in data.columns_mut() {
        let (rest, elevations) = parse_dted_record(n_lats, input)?;
        input = rest;
        col.assign(&elevations);
    }

    Ok((input, data))
}

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
    fn test_to_i16() {
        let x = 0b0000000000000000;
        assert_eq!(super::to_i16(x), 0);

        let x = 0b0000000000000001;
        assert_eq!(super::to_i16(x), 1);

        let x = 0b0000000000000010;
        assert_eq!(super::to_i16(x), 2);

        let x = 0b0010000000000000;
        assert_eq!(super::to_i16(x), 8192);

        let x = 0b0100000000000000;
        assert_eq!(super::to_i16(x), 16384);

        let x = 0b1000000000000000;
        assert_eq!(super::to_i16(x), 0);

        let x = 0b1000000000000001;
        assert_eq!(super::to_i16(x), -1);

        let x = 0b1000000000000010;
        assert_eq!(super::to_i16(x), -2);

        let x = 0b1010000000000000;
        assert_eq!(super::to_i16(x), -8192);

        let x = 0b1100000000000000;
        assert_eq!(super::to_i16(x), -16384);
    }
}
