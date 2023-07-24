use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum CoordinateError {
    #[error("Latitude out of range")]
    LatitudeOutOfRange,
    #[error("Longitude out of range")]
    LongitudeOutOfRange,
}

#[derive(Debug, Copy, Clone)]
pub struct Coordinate2d {
    lat: f64, // latitude in degrees (normalised to 0-1)
    lon: f64, // longitude in degrees (normalised to 0-1)
}

impl Coordinate2d {
    /// Create a coordinate from a latitude [0, 1] and longitude [0, 1]
    pub fn new(lat: f64, lon: f64) -> Result<Self, CoordinateError> {
        if !(0.0..=1.0).contains(&lat) {
            return Err(CoordinateError::LatitudeOutOfRange);
        }
        if !(0.0..=1.0).contains(&lon) {
            return Err(CoordinateError::LongitudeOutOfRange);
        }
        Ok(Self { lat, lon })
    }

    /// Create a coordinate from a latitude [-90, 90] and longitude [-180, 180]
    pub fn from_degrees(lat: f64, lon: f64) -> Result<Self, CoordinateError> {
        Self::new((lat + 90.0) / 180.0, (lon + 180.0) / 360.0)
    }

    pub fn lat_deg(&self) -> f64 {
        self.lat * 180.0 - 90.0
    }

    pub fn lon_deg(&self) -> f64 {
        self.lon * 360.0 - 180.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate2d_new() -> Result<(), CoordinateError> {
        let c = Coordinate2d::new(0.5, 0.5)?;
        assert_eq!(c.lat_deg(), 0.0);
        assert_eq!(c.lon_deg(), 0.0);

        let c = Coordinate2d::new(1.0, 1.0)?;
        assert_eq!(c.lat_deg(), 90.0);
        assert_eq!(c.lon_deg(), 180.0);

        Ok(())
    }

    #[test]
    fn test_coordinate2d_from_degrees() -> Result<(), CoordinateError> {
        let c = Coordinate2d::from_degrees(0.0, 0.0)?;
        assert_eq!(c.lat, 0.5);
        assert_eq!(c.lon, 0.5);

        let c = Coordinate2d::from_degrees(-90.0, -180.0)?;
        assert_eq!(c.lat, 0.0);
        assert_eq!(c.lon, 0.0);

        let c = Coordinate2d::from_degrees(90.0, 180.0)?;
        assert_eq!(c.lat, 1.0);
        assert_eq!(c.lon, 1.0);

        let c = Coordinate2d::from_degrees(-180.0, -360.0);
        assert!(c.is_err());
        assert_eq!(c.unwrap_err(), CoordinateError::LatitudeOutOfRange);

        let c = Coordinate2d::from_degrees(180.0, 360.0);
        assert!(c.is_err());
        assert_eq!(c.unwrap_err(), CoordinateError::LatitudeOutOfRange);

        let c = Coordinate2d::from_degrees(-90.0, -180.1);
        assert!(c.is_err());
        assert_eq!(c.unwrap_err(), CoordinateError::LongitudeOutOfRange);

        let c = Coordinate2d::from_degrees(90.0, 180.1);
        assert!(c.is_err());
        assert_eq!(c.unwrap_err(), CoordinateError::LongitudeOutOfRange);

        Ok(())
    }
}
