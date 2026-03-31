//! StatusOr utilities.
//! Reference: `_ref/draco/src/draco/core/status_or.h`.

use crate::core::status::{ok_status, Status};

#[derive(Clone, Debug)]
pub struct StatusOr<T> {
    status: Status,
    value: Option<T>,
}

impl<T> StatusOr<T> {
    pub fn new_status(status: Status) -> Self {
        debug_assert!(
            !status.is_ok(),
            "StatusOr::new_status() should not be used with an OK status"
        );
        Self {
            status,
            value: None,
        }
    }

    pub fn new_value(value: T) -> Self {
        Self {
            status: ok_status(),
            value: Some(value),
        }
    }

    pub fn new(status: Status, value: T) -> Self {
        if status.is_ok() {
            Self {
                status,
                value: Some(value),
            }
        } else {
            Self {
                status,
                value: None,
            }
        }
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn value(&self) -> &T {
        self.value_or_die()
    }

    pub fn value_mut(&mut self) -> &mut T {
        self.value.as_mut().expect("StatusOr is not OK")
    }

    pub fn into_value(self) -> T {
        self.into_result().expect("StatusOr is not OK")
    }

    pub fn into_result(self) -> Result<T, Status> {
        match (self.status.is_ok(), self.value) {
            (true, Some(value)) => Ok(value),
            (_, _) => Err(self.status),
        }
    }

    pub fn from_result(result: Result<T, Status>) -> Self {
        match result {
            Ok(value) => Self::new_value(value),
            Err(status) => Self::new_status(status),
        }
    }

    pub fn value_or_die(&self) -> &T {
        self.value.as_ref().expect("StatusOr is not OK")
    }

    pub fn ok(&self) -> bool {
        self.status.is_ok()
    }

    pub fn is_ok(&self) -> bool {
        self.ok()
    }

    pub fn has_value(&self) -> bool {
        self.value.is_some()
    }
}

#[macro_export]
macro_rules! draco_assign_or_return {
    ($lhs:expr, $expression:expr) => {{
        let _statusor = $expression;
        if !_statusor.ok() {
            return _statusor.status().clone();
        }
        $lhs = _statusor.into_result().expect("StatusOr missing value");
    }};
}

#[cfg(test)]
mod tests {
    use super::StatusOr;
    use crate::core::status::{ok_status, Status, StatusCode};

    #[test]
    fn error_status_has_no_value() {
        let status = Status::new(StatusCode::DracoError, "error");
        let status_or = StatusOr::<i32>::new_status(status.clone());

        assert!(!status_or.ok());
        assert!(!status_or.has_value());
        assert_eq!(status_or.status(), &status);
        assert!(status_or.into_result().is_err());
    }

    #[test]
    fn non_ok_new_drops_value_payload() {
        let status = Status::new(StatusCode::DracoError, "error");
        let status_or = StatusOr::new(status.clone(), 7);

        assert!(!status_or.ok());
        assert!(!status_or.has_value());
        assert_eq!(status_or.status(), &status);
    }

    #[test]
    fn ok_value_round_trips_through_result_surface() {
        let status_or = StatusOr::new(ok_status(), 42);

        assert!(status_or.ok());
        assert!(status_or.has_value());
        assert_eq!(*status_or.value(), 42);
        assert_eq!(status_or.into_value(), 42);

        let from_result = StatusOr::from_result(Ok::<_, Status>(9));
        assert!(from_result.ok());
        assert_eq!(from_result.into_result().expect("value"), 9);
    }
}
