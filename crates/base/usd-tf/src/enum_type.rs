//! Type-erased enum wrapper.
//!
//! TfEnum provides a way to store enum values while preserving their type
//! information at runtime, enabling type discrimination between enums with
//! the same underlying values.
//!
//! # Examples
//!
//! ```
//! use usd_tf::TfEnum;
//!
//! #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
//! #[repr(i32)]
//! enum Monster { Sulley = 0, Mike = 1, Roz = 2 }
//! impl From<Monster> for i32 { fn from(m: Monster) -> i32 { m as i32 } }
//!
//! #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
//! #[repr(i32)]
//! enum Fish { Nemo = 0, Father = 1, Dory = 2 }
//! impl From<Fish> for i32 { fn from(f: Fish) -> i32 { f as i32 } }
//!
//! let t1 = TfEnum::new(Monster::Mike);
//! let t2 = TfEnum::new(Fish::Nemo);
//!
//! assert!(t1.is_a::<Monster>());
//! assert!(!t1.is_a::<Fish>());
//! assert_ne!(t1, t2); // Different types, even if same underlying value
//! ```

use std::any::TypeId;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::RwLock;

/// Global registry for enum names.
struct EnumRegistry {
    /// Maps (TypeId, value) -> short name
    enum_to_name: HashMap<(TypeId, i32), String>,
    /// Maps (TypeId, value) -> full name (TypeName::ValueName)
    enum_to_full_name: HashMap<(TypeId, i32), String>,
    /// Maps (TypeId, value) -> display name
    enum_to_display_name: HashMap<(TypeId, i32), String>,
    /// Maps full name -> (TypeId, value)
    full_name_to_enum: HashMap<String, (TypeId, i32)>,
    /// Maps type_name -> Vec<short names>
    type_name_to_names: HashMap<String, Vec<String>>,
    /// Maps type_name -> TypeId
    type_name_to_type: HashMap<String, TypeId>,
}

impl EnumRegistry {
    fn new() -> Self {
        Self {
            enum_to_name: HashMap::new(),
            enum_to_full_name: HashMap::new(),
            enum_to_display_name: HashMap::new(),
            full_name_to_enum: HashMap::new(),
            type_name_to_names: HashMap::new(),
            type_name_to_type: HashMap::new(),
        }
    }
}

static REGISTRY: RwLock<Option<EnumRegistry>> = RwLock::new(None);

fn with_registry<F, R>(f: F) -> R
where
    F: FnOnce(&EnumRegistry) -> R,
{
    let guard = REGISTRY.read().expect("enum registry lock poisoned");
    if let Some(ref reg) = *guard {
        f(reg)
    } else {
        drop(guard);
        let mut guard = REGISTRY.write().expect("enum registry lock poisoned");
        if guard.is_none() {
            *guard = Some(EnumRegistry::new());
        }
        drop(guard);
        let guard = REGISTRY.read().expect("enum registry lock poisoned");
        f(guard.as_ref().expect("registry just initialized"))
    }
}

fn with_registry_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut EnumRegistry) -> R,
{
    let mut guard = REGISTRY.write().expect("enum registry lock poisoned");
    if guard.is_none() {
        *guard = Some(EnumRegistry::new());
    }
    f(guard.as_mut().expect("registry just initialized"))
}

/// Type-erased enum value with runtime type discrimination.
///
/// TfEnum can hold any enum value while preserving type information,
/// allowing runtime type checks and comparisons between enums of
/// different types.
#[derive(Clone, Copy)]
pub struct TfEnum {
    type_id: TypeId,
    value: i32,
}

impl TfEnum {
    /// Creates a new TfEnum from an enum value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::TfEnum;
    ///
    /// #[derive(Clone, Copy)]
    /// #[repr(i32)]
    /// enum Color { Red = 0, Green = 1, Blue = 2 }
    /// impl From<Color> for i32 { fn from(c: Color) -> i32 { c as i32 } }
    ///
    /// let e = TfEnum::new(Color::Green);
    /// assert_eq!(e.value(), 1);
    /// ```
    pub fn new<T: 'static + Copy + Into<i32>>(value: T) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            value: value.into(),
        }
    }

    /// Creates a TfEnum from a raw TypeId and value.
    ///
    /// # Safety
    ///
    /// Caller must ensure that the TypeId corresponds to an actual enum type
    /// and that value is a valid value for that enum.
    pub fn from_raw(type_id: TypeId, value: i32) -> Self {
        Self { type_id, value }
    }

    /// Creates a TfEnum holding an integer value.
    ///
    /// This creates a TfEnum that is typed as `i32`.
    #[must_use]
    pub fn from_int(value: i32) -> Self {
        Self {
            type_id: TypeId::of::<i32>(),
            value,
        }
    }

    /// Returns true if this TfEnum holds a value of type T.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::TfEnum;
    ///
    /// #[derive(Clone, Copy)]
    /// #[repr(i32)]
    /// enum A { X = 0 }
    /// impl From<A> for i32 { fn from(a: A) -> i32 { a as i32 } }
    /// #[derive(Clone, Copy)]
    /// #[repr(i32)]
    /// enum B { Y = 0 }
    /// impl From<B> for i32 { fn from(b: B) -> i32 { b as i32 } }
    ///
    /// let e = TfEnum::new(A::X);
    /// assert!(e.is_a::<A>());
    /// assert!(!e.is_a::<B>());
    /// ```
    #[must_use]
    pub fn is_a<T: 'static>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }

    /// Returns true if this TfEnum holds the given TypeId.
    #[must_use]
    pub fn is_type(&self, type_id: TypeId) -> bool {
        self.type_id == type_id
    }

    /// Returns the TypeId of the stored enum type.
    #[must_use]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns the integral value of the enum.
    #[must_use]
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Gets the value as type T.
    ///
    /// # Panics
    ///
    /// Panics if this TfEnum does not hold a value of type T.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::TfEnum;
    ///
    /// #[derive(Clone, Copy, PartialEq, Debug)]
    /// #[repr(i32)]
    /// enum Num { One = 1, Two = 2 }
    ///
    /// impl From<i32> for Num {
    ///     fn from(v: i32) -> Self {
    ///         match v { 1 => Num::One, 2 => Num::Two, _ => panic!() }
    ///     }
    /// }
    /// impl From<Num> for i32 {
    ///     fn from(n: Num) -> i32 { n as i32 }
    /// }
    ///
    /// let e = TfEnum::new(Num::Two);
    /// assert_eq!(e.get::<Num>(), Num::Two);
    /// ```
    #[must_use]
    pub fn get<T: 'static + From<i32>>(&self) -> T {
        assert!(self.is_a::<T>(), "TfEnum::get: type mismatch");
        T::from(self.value)
    }

    /// Gets the value as type T, returning None if types don't match.
    #[must_use]
    pub fn try_get<T: 'static + From<i32>>(&self) -> Option<T> {
        if self.is_a::<T>() {
            Some(T::from(self.value))
        } else {
            None
        }
    }

    /// Registers a name for an enum value.
    ///
    /// # Arguments
    ///
    /// * `type_name` - The name of the enum type (e.g., "Season")
    /// * `val_name` - The name of the enum value (e.g., "WINTER")
    /// * `display_name` - Optional display name for UI purposes
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::TfEnum;
    ///
    /// #[derive(Clone, Copy)]
    /// #[repr(i32)]
    /// enum Season { Spring = 0, Summer = 1, Autumn = 2, Winter = 3 }
    ///
    /// impl From<Season> for i32 { fn from(s: Season) -> i32 { s as i32 } }
    ///
    /// TfEnum::add_name::<Season>(Season::Spring as i32, "Season", "Spring", None);
    /// TfEnum::add_name::<Season>(Season::Winter as i32, "Season", "Winter", Some("Cold Season"));
    /// ```
    pub fn add_name<T: 'static>(
        value: i32,
        type_name: &str,
        val_name: &str,
        display_name: Option<&str>,
    ) {
        // Strip any leading namespace/scope (e.g., "mod::VALUE" -> "VALUE")
        let short_name = val_name
            .rfind(':')
            .map(|i| &val_name[i + 1..])
            .unwrap_or(val_name);

        if short_name.is_empty() {
            return;
        }

        let type_id = TypeId::of::<T>();
        let full_name = format!("{}::{}", type_name, short_name);
        let display = display_name
            .map(String::from)
            .unwrap_or_else(|| short_name.to_string());

        with_registry_mut(|r| {
            let key = (type_id, value);
            r.enum_to_name.insert(key, short_name.to_string());
            r.enum_to_full_name.insert(key, full_name.clone());
            r.enum_to_display_name.insert(key, display);
            r.full_name_to_enum.insert(full_name, key);
            r.type_name_to_names
                .entry(type_name.to_string())
                .or_default()
                .push(short_name.to_string());
            r.type_name_to_type.insert(type_name.to_string(), type_id);
        });
    }

    /// Returns the short name for an enum value.
    ///
    /// If the value is an integer type, returns the integer as a string.
    /// Returns an empty string if no name is registered.
    #[must_use]
    pub fn get_name(&self) -> String {
        if self.type_id == TypeId::of::<i32>() {
            return self.value.to_string();
        }

        with_registry(|r| {
            r.enum_to_name
                .get(&(self.type_id, self.value))
                .cloned()
                .unwrap_or_default()
        })
    }

    /// Returns the fully-qualified name (e.g., "Season::WINTER").
    ///
    /// For integer types, returns "int::value".
    /// Returns an empty string if no name is registered.
    #[must_use]
    pub fn get_full_name(&self) -> String {
        if self.type_id == TypeId::of::<i32>() {
            return format!("int::{}", self.value);
        }

        with_registry(|r| {
            r.enum_to_full_name
                .get(&(self.type_id, self.value))
                .cloned()
                .unwrap_or_default()
        })
    }

    /// Returns the display name for UI purposes.
    ///
    /// Falls back to the short name if no display name was registered.
    #[must_use]
    pub fn get_display_name(&self) -> String {
        if self.type_id == TypeId::of::<i32>() {
            return self.value.to_string();
        }

        with_registry(|r| {
            r.enum_to_display_name
                .get(&(self.type_id, self.value))
                .cloned()
                .unwrap_or_default()
        })
    }

    /// Returns all registered names for a given enum type.
    #[must_use]
    pub fn get_all_names<T: 'static>(type_name: &str) -> Vec<String> {
        if TypeId::of::<T>() == TypeId::of::<i32>() {
            return Vec::new();
        }

        with_registry(|r| {
            r.type_name_to_names
                .get(type_name)
                .cloned()
                .unwrap_or_default()
        })
    }

    /// Returns the TypeId for a given type name.
    #[must_use]
    pub fn get_type_from_name(type_name: &str) -> Option<TypeId> {
        with_registry(|r| r.type_name_to_type.get(type_name).copied())
    }

    /// Returns the enum value for a short name within a given type.
    ///
    /// # Returns
    ///
    /// Returns Some(value) if found, None otherwise.
    #[must_use]
    pub fn get_value_from_name<T: 'static>(type_name: &str, name: &str) -> Option<i32> {
        let full_name = format!("{}::{}", type_name, name);
        Self::get_value_from_full_name(&full_name).and_then(|(tid, val)| {
            if tid == TypeId::of::<T>() {
                Some(val)
            } else {
                None
            }
        })
    }

    /// Returns the (TypeId, value) for a fully-qualified name.
    ///
    /// Handles special case "int::N" for integer values.
    #[must_use]
    pub fn get_value_from_full_name(full_name: &str) -> Option<(TypeId, i32)> {
        // Handle "int::N" special case
        if let Some(int_str) = full_name.strip_prefix("int::") {
            if let Ok(v) = int_str.parse::<i32>() {
                return Some((TypeId::of::<i32>(), v));
            }
        }

        with_registry(|r| r.full_name_to_enum.get(full_name).copied())
    }

    /// Returns true if the given type name has been registered.
    #[must_use]
    pub fn is_known_enum_type(type_name: &str) -> bool {
        with_registry(|r| r.type_name_to_type.contains_key(type_name))
    }
}

impl Default for TfEnum {
    /// Default TfEnum holds integer value 0.
    fn default() -> Self {
        Self::from_int(0)
    }
}

impl PartialEq for TfEnum {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id && self.value == other.value
    }
}

impl Eq for TfEnum {}

impl PartialOrd for TfEnum {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TfEnum {
    fn cmp(&self, other: &Self) -> Ordering {
        // First compare by type (using hash for consistent ordering)
        let self_type_hash = {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            self.type_id.hash(&mut h);
            h.finish()
        };
        let other_type_hash = {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            other.type_id.hash(&mut h);
            h.finish()
        };

        match self_type_hash.cmp(&other_type_hash) {
            Ordering::Equal => self.value.cmp(&other.value),
            ord => ord,
        }
    }
}

impl Hash for TfEnum {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.type_id.hash(state);
        self.value.hash(state);
    }
}

impl fmt::Debug for TfEnum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.get_full_name();
        if name.is_empty() {
            write!(f, "TfEnum({:?}, {})", self.type_id, self.value)
        } else {
            write!(f, "TfEnum({})", name)
        }
    }
}

impl fmt::Display for TfEnum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.get_full_name();
        if name.is_empty() {
            write!(f, "{}", self.value)
        } else {
            write!(f, "{}", name)
        }
    }
}

impl From<i32> for TfEnum {
    fn from(value: i32) -> Self {
        Self::from_int(value)
    }
}

/// Macro to register enum names.
///
/// # Examples
///
/// ```ignore
/// enum Season { Spring, Summer, Autumn, Winter }
///
/// tf_add_enum_name!(Season, Spring);
/// tf_add_enum_name!(Season, Summer, "Hot Season");
/// ```
#[macro_export]
macro_rules! tf_add_enum_name {
    ($enum_type:ty, $value:ident) => {
        $crate::TfEnum::add_name::<$enum_type>(
            <$enum_type>::$value as i32,
            stringify!($enum_type),
            stringify!($value),
            None,
        );
    };
    ($enum_type:ty, $value:ident, $display:expr) => {
        $crate::TfEnum::add_name::<$enum_type>(
            <$enum_type>::$value as i32,
            stringify!($enum_type),
            stringify!($value),
            Some($display),
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    #[repr(i32)]
    enum Monster {
        Sulley = 0,
        Mike = 1,
        Roz = 2,
    }

    impl From<Monster> for i32 {
        fn from(m: Monster) -> i32 {
            m as i32
        }
    }

    impl From<i32> for Monster {
        fn from(v: i32) -> Self {
            match v {
                0 => Monster::Sulley,
                1 => Monster::Mike,
                2 => Monster::Roz,
                _ => panic!("Invalid Monster value"),
            }
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    #[repr(i32)]
    enum Fish {
        Nemo = 0,
        Father = 1,
        Dory = 2,
    }

    impl From<Fish> for i32 {
        fn from(f: Fish) -> i32 {
            f as i32
        }
    }

    impl From<i32> for Fish {
        fn from(v: i32) -> Self {
            match v {
                0 => Fish::Nemo,
                1 => Fish::Father,
                2 => Fish::Dory,
                _ => panic!("Invalid Fish value"),
            }
        }
    }

    #[test]
    fn test_type_discrimination() {
        // Sulley=0 and Nemo=0 have same underlying value
        let t1 = TfEnum::new(Monster::Sulley);
        let t2 = TfEnum::new(Fish::Nemo);

        assert!(t1.is_a::<Monster>());
        assert!(!t1.is_a::<Fish>());
        assert!(t2.is_a::<Fish>());
        assert!(!t2.is_a::<Monster>());

        // Same value (0), different types - must be not equal
        assert_ne!(t1, t2);
    }

    #[test]
    fn test_equality() {
        let e1 = TfEnum::new(Monster::Mike);
        let e2 = TfEnum::new(Monster::Mike);
        let e3 = TfEnum::new(Monster::Sulley);

        assert_eq!(e1, e2);
        assert_ne!(e1, e3);
    }

    #[test]
    fn test_get_value() {
        let e = TfEnum::new(Monster::Roz);
        assert_eq!(e.value(), 2);
        assert_eq!(e.get::<Monster>(), Monster::Roz);
    }

    #[test]
    fn test_try_get() {
        let e = TfEnum::new(Monster::Mike);
        assert!(e.try_get::<Monster>().is_some());
        assert!(e.try_get::<Fish>().is_none());
    }

    #[test]
    fn test_from_int() {
        let e = TfEnum::from_int(42);
        assert!(e.is_a::<i32>());
        assert_eq!(e.value(), 42);
    }

    #[test]
    fn test_default() {
        let e = TfEnum::default();
        assert!(e.is_a::<i32>());
        assert_eq!(e.value(), 0);
    }

    #[test]
    fn test_name_registration() {
        TfEnum::add_name::<Monster>(Monster::Sulley as i32, "Monster", "Sulley", None);
        TfEnum::add_name::<Monster>(
            Monster::Mike as i32,
            "Monster",
            "Mike",
            Some("Mike Wazowski"),
        );

        let e = TfEnum::new(Monster::Mike);
        assert_eq!(e.get_name(), "Mike");
        assert_eq!(e.get_full_name(), "Monster::Mike");
        assert_eq!(e.get_display_name(), "Mike Wazowski");

        let e2 = TfEnum::new(Monster::Sulley);
        assert_eq!(e2.get_name(), "Sulley");
        assert_eq!(e2.get_display_name(), "Sulley"); // No display name, falls back to short name
    }

    #[test]
    fn test_int_name() {
        let e = TfEnum::from_int(123);
        assert_eq!(e.get_name(), "123");
        assert_eq!(e.get_full_name(), "int::123");
    }

    #[test]
    fn test_get_value_from_full_name() {
        TfEnum::add_name::<Fish>(Fish::Dory as i32, "Fish", "Dory", None);

        let result = TfEnum::get_value_from_full_name("Fish::Dory");
        assert!(result.is_some());
        let (type_id, value) = result.unwrap();
        assert_eq!(type_id, TypeId::of::<Fish>());
        assert_eq!(value, Fish::Dory as i32);

        // Test int::N format
        let int_result = TfEnum::get_value_from_full_name("int::42");
        assert!(int_result.is_some());
        let (type_id, value) = int_result.unwrap();
        assert_eq!(type_id, TypeId::of::<i32>());
        assert_eq!(value, 42);
    }

    #[test]
    fn test_get_all_names() {
        TfEnum::add_name::<Monster>(Monster::Roz as i32, "Monster", "Roz", None);
        let names = TfEnum::get_all_names::<Monster>("Monster");
        assert!(names.contains(&"Roz".to_string()));
    }

    #[test]
    fn test_is_known_enum_type() {
        TfEnum::add_name::<Fish>(Fish::Father as i32, "Fish", "Father", None);
        assert!(TfEnum::is_known_enum_type("Fish"));
        assert!(!TfEnum::is_known_enum_type("UnknownType"));
    }

    #[test]
    fn test_ordering() {
        let e1 = TfEnum::new(Monster::Sulley);
        let e2 = TfEnum::new(Monster::Mike);
        // Within same type, ordering is by value
        assert!(e1 < e2);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(TfEnum::new(Monster::Mike));
        set.insert(TfEnum::new(Fish::Nemo));
        set.insert(TfEnum::new(Monster::Sulley));

        assert!(set.contains(&TfEnum::new(Monster::Mike)));
        assert!(!set.contains(&TfEnum::new(Monster::Roz)));
    }

    #[test]
    fn test_display() {
        TfEnum::add_name::<Fish>(Fish::Nemo as i32, "Fish", "Nemo", None);
        let e = TfEnum::new(Fish::Nemo);
        assert_eq!(format!("{}", e), "Fish::Nemo");

        let int_e = TfEnum::from_int(99);
        assert_eq!(format!("{}", int_e), "int::99");
    }
}
