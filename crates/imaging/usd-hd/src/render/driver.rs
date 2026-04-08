//! HdDriver - GPU device handle wrapper.
//!
//! Represents a device object (typically a render device) owned by the application
//! and passed to HdRenderIndex. The render index passes it to the render delegate
//! and rendering tasks.
//!
//! The application manages the lifetime and must ensure validity while Hydra is running.

use usd_tf::Token;
use usd_vt::Value;

/// Device object wrapper for passing GPU/render context to Hydra.
///
/// # Example
/// ```ignore
/// use usd_hd::render::HdDriver;
/// use usd_tf::Token;
/// use usd_vt::Value;
///
/// // Create a driver for a rendering backend
/// let driver = HdDriver::new(
///     Token::new("renderDriver"),
///     Value::from(my_gpu_device)
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdDriver {
    /// Driver identifier (e.g., "renderDriver", "hgi")
    pub name: Token,

    /// Opaque driver handle
    pub driver: Value,
}

impl HdDriver {
    /// Create a new driver with name and handle.
    pub fn new(name: Token, driver: Value) -> Self {
        Self { name, driver }
    }

    /// Get driver name.
    pub fn name(&self) -> &Token {
        &self.name
    }

    /// Get driver handle.
    pub fn driver(&self) -> &Value {
        &self.driver
    }

    /// Get mutable driver handle.
    pub fn driver_mut(&mut self) -> &mut Value {
        &mut self.driver
    }
}

/// Vector of driver pointers for passing multiple drivers.
pub type HdDriverVector = Vec<HdDriver>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_creation() {
        let name = Token::new("testDriver");
        let value = Value::from(42i32);
        let driver = HdDriver::new(name.clone(), value);

        assert_eq!(driver.name().as_str(), "testDriver");
    }

    #[test]
    fn test_driver_vector() {
        let drivers: HdDriverVector = vec![
            HdDriver::new(Token::new("driver1"), Value::from(1i32)),
            HdDriver::new(Token::new("driver2"), Value::from(2i32)),
        ];

        assert_eq!(drivers.len(), 2);
    }
}
