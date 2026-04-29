use crate::duration::Duration;

/// A silent musical event with a duration.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rest {
    duration: Duration,
}

impl Rest {
    /// Creates a rest with the supplied duration.
    pub fn new(duration: Duration) -> Self {
        Self { duration }
    }

    /// Creates a rest from a quarter-length value.
    pub fn from_quarter_length(quarter_length: crate::FloatType) -> crate::Result<Self> {
        Ok(Self::new(Duration::new(quarter_length)?))
    }

    /// Returns the rest duration.
    pub fn duration(&self) -> &Duration {
        &self.duration
    }

    /// Updates the rest duration.
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = duration;
    }

    /// Returns the duration in quarter lengths.
    pub fn quarter_length(&self) -> crate::FloatType {
        self.duration.quarter_length()
    }
}

impl Default for Rest {
    fn default() -> Self {
        Self::new(Duration::default())
    }
}

impl From<Duration> for Rest {
    fn from(value: Duration) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_has_duration() {
        let rest = Rest::from_quarter_length(2.0).unwrap();
        assert_eq!(rest.quarter_length(), 2.0);
    }

    #[test]
    fn rest_supports_default_from_and_updates() {
        let mut rest = Rest::default();
        assert_eq!(rest.quarter_length(), 1.0);

        rest.set_duration(Duration::whole());
        assert_eq!(rest.duration(), &Duration::whole());

        let half_rest = Rest::from(Duration::half());
        assert_eq!(half_rest.quarter_length(), 2.0);
        assert!(Rest::from_quarter_length(crate::FloatType::NAN).is_err());
    }
}
