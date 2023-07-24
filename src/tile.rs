use super::coordinate_2d::Coordinate2d;
use super::parser::{parse_dted_tile, ParseError};

use ndarray::Array2;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

// DTED is a format for storing elevation data. It's a binary format, but it's
// pretty simple. The spec is available here:
// https://geoservice.dlr.de/web/dataguide/srtm/pdfs/SRTM-XSAR-DEM-DTED-1.1.pdf

pub struct DtedHeader {
    pub(crate) origin_sw: Coordinate2d,
    pub(crate) num_lat_points: usize,
    pub(crate) num_lon_points: usize,
}

impl DtedHeader {
    pub fn origin(&self) -> Coordinate2d {
        self.origin_sw
    }

    pub fn num_lat(&self) -> usize {
        self.num_lat_points
    }

    pub fn num_lon(&self) -> usize {
        self.num_lon_points
    }
}

pub struct DtedTile {
    pub(crate) header: DtedHeader,
    pub(crate) data: Array2<i16>,
}

impl DtedTile {
    pub fn from_bytes(bytes: &[u8]) -> Result<DtedTile, ParseError> {
        let (_, tile) =
            parse_dted_tile(bytes).map_err(|e| ParseError::Invalid(format!("{}", e)))?;
        Ok(tile)
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<DtedTile, ParseError> {
        let file = File::open(path).map_err(ParseError::Io)?;
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).map_err(ParseError::Io)?;
        Self::from_bytes(&buffer)
    }

    pub fn header(&self) -> &DtedHeader {
        &self.header
    }

    pub fn min_lon_deg(&self) -> f64 {
        self.header.origin_sw.lon_deg()
    }

    pub fn max_lon_deg(&self) -> f64 {
        self.header.origin_sw.lon_deg() + 1.0
    }

    pub fn min_lat_deg(&self) -> f64 {
        self.header.origin_sw.lat_deg()
    }

    pub fn max_lat_deg(&self) -> f64 {
        self.header.origin_sw.lat_deg() + 1.0
    }

    pub fn contains(&self, coord: Coordinate2d) -> bool {
        let is_within_lat = (self.min_lat_deg()..=self.max_lat_deg()).contains(&coord.lat_deg());
        let is_within_lon = (self.min_lon_deg()..=self.max_lon_deg()).contains(&coord.lon_deg());

        is_within_lat && is_within_lon
    }

    pub fn elevation_m(&self, coord: Coordinate2d) -> Option<i16> {
        if !self.contains(coord) {
            return None;
        }

        let lon_index = (coord.lon_deg() - self.min_lon_deg()) * self.header.num_lon() as f64;
        let lat_index = (coord.lat_deg() - self.min_lat_deg()) * self.header.num_lat() as f64;
        let lon_index = lon_index as usize;
        let lat_index = lat_index as usize;
        Some(self.data[[lat_index, lon_index]])
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use anyhow::Result;
    use std::path::Path;

    #[test]
    fn test_from_file() -> Result<()> {
        let resource_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
        let tile = DtedTile::from_file(resource_dir.join("n47.dt2"))?;

        assert_eq!(tile.header.origin_sw.lat_deg(), 47.0);
        assert_eq!(tile.header.origin_sw.lon_deg(), 8.0);
        assert_eq!(tile.header.num_lat_points, 3601);
        assert_eq!(tile.header.num_lon_points, 3601);

        let coordinates = vec![
            Coordinate2d::from_degrees(47.356418477, 8.5189232237)?,
            Coordinate2d::from_degrees(47.349792968, 8.4909410835)?,
            Coordinate2d::from_degrees(47.164800109, 8.6838999052)?,
            Coordinate2d::from_degrees(47.310359476, 8.9664085558)?,
        ];

        let elevations = vec![421.0, 871.0, 1116.0, 857.0];

        for (coord, elevation) in coordinates.iter().zip(elevations.iter()) {
            assert_eq!(tile.elevation_m(*coord).unwrap(), *elevation as i16);
        }

        Ok(())
    }
}
