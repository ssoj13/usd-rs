use usd_tf::{TfEnum, tf_add_enum_name};

// Condiment enum mirrors the C++ test enum.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i32)]
enum Condiment {
    Salt = 0,
    Pepper = 13,
    Ketchup = 14,
    NoName = 15,
}

impl From<Condiment> for i32 {
    fn from(c: Condiment) -> i32 {
        c as i32
    }
}

impl From<i32> for Condiment {
    fn from(v: i32) -> Self {
        match v {
            0 => Condiment::Salt,
            13 => Condiment::Pepper,
            14 => Condiment::Ketchup,
            15 => Condiment::NoName,
            _ => Condiment::NoName,
        }
    }
}

// Season enum mirrors the C++ test enum (SUMMER has explicit initializer 3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i32)]
enum Season {
    Spring = 0,
    Summer = 3,
    Autumn = 4,
    Winter = 5,
}

impl From<Season> for i32 {
    fn from(s: Season) -> i32 {
        s as i32
    }
}

impl From<i32> for Season {
    fn from(v: i32) -> Self {
        match v {
            0 => Season::Spring,
            3 => Season::Summer,
            4 => Season::Autumn,
            5 => Season::Winter,
            _ => Season::Spring,
        }
    }
}

// Register all names once before any test that needs them.
fn register_condiment_names() {
    tf_add_enum_name!(Condiment, Salt, "Salt");
    tf_add_enum_name!(Condiment, Pepper, "Pepper");
    tf_add_enum_name!(Condiment, Ketchup, "Ketchup");
    // NoName deliberately omitted — mirrors C++ where NO_NAME has no display name
}

fn register_season_names() {
    tf_add_enum_name!(Season, Spring);
    tf_add_enum_name!(Season, Summer);
    tf_add_enum_name!(Season, Autumn);
    tf_add_enum_name!(Season, Winter);
}

// GetName returns the short name for a registered value.
#[test]
fn get_name_pepper() {
    register_condiment_names();
    let e = TfEnum::new(Condiment::Pepper);
    assert_eq!(e.get_name(), "Pepper");
}

// GetFullName returns "TypeName::ValueName".
#[test]
fn get_full_name_pepper() {
    register_condiment_names();
    let e = TfEnum::new(Condiment::Pepper);
    assert_eq!(e.get_full_name(), "Condiment::Pepper");
}

// GetDisplayName returns the registered display name.
#[test]
fn get_display_name_pepper() {
    register_condiment_names();
    let e = TfEnum::new(Condiment::Pepper);
    assert_eq!(e.get_display_name(), "Pepper");
}

// GetValueFromName returns the value and sets found=true for known names.
#[test]
fn get_value_from_name_ketchup_found() {
    register_condiment_names();
    let val = TfEnum::get_value_from_name::<Condiment>("Condiment", "Ketchup");
    assert!(val.is_some());
    let condiment: Condiment = val.unwrap().into();
    assert_eq!(condiment, Condiment::Ketchup);
}

// GetValueFromFullName handles "TypeName::ValueName" form.
#[test]
fn get_value_from_full_name_ketchup_found() {
    register_condiment_names();
    let result = TfEnum::get_value_from_full_name("Condiment::Ketchup");
    assert!(result.is_some());
    let (_tid, val) = result.unwrap();
    assert_eq!(val, Condiment::Ketchup as i32);
}

// GetName for unregistered value returns empty string.
#[test]
fn get_name_no_name_unregistered() {
    register_condiment_names();
    let e = TfEnum::new(Condiment::NoName);
    // NoName was not registered, so short name should be empty
    assert_eq!(e.get_name(), "");
}

// GetValueFromName for unknown name returns None (found=false).
#[test]
fn get_value_from_name_squid_not_found() {
    register_condiment_names();
    let val = TfEnum::get_value_from_name::<Condiment>("Condiment", "SQUID");
    assert!(val.is_none());
}

// GetValueFromFullName for unknown full name returns None (found=false).
#[test]
fn get_value_from_full_name_squid_not_found() {
    register_condiment_names();
    let result = TfEnum::get_value_from_full_name("Condiment::SQUID");
    assert!(result.is_none());
}

// GetName returns "SUMMER" for Season::Summer.
#[test]
fn get_name_summer() {
    register_season_names();
    let e = TfEnum::new(Season::Summer);
    assert_eq!(e.get_name(), "Summer");
}

// GetFullName returns "Season::SUMMER".
#[test]
fn get_full_name_summer() {
    register_season_names();
    let e = TfEnum::new(Season::Summer);
    assert_eq!(e.get_full_name(), "Season::Summer");
}

// GetDisplayName falls back to short name when no display name is given.
#[test]
fn get_display_name_summer_fallback() {
    register_season_names();
    let e = TfEnum::new(Season::Summer);
    // No display name registered for Summer, should equal short name
    assert_eq!(e.get_display_name(), e.get_name());
}

// GetValueFromName for Season::Autumn returns found=true and correct value.
#[test]
fn get_value_from_name_autumn_found() {
    register_season_names();
    let val = TfEnum::get_value_from_name::<Season>("Season", "Autumn");
    assert!(val.is_some());
    let season: Season = val.unwrap().into();
    assert_eq!(season, Season::Autumn);
}

// GetValueFromName for unknown value returns None (found=false).
#[test]
fn get_value_from_name_monday_not_found() {
    register_season_names();
    let val = TfEnum::get_value_from_name::<Season>("Season", "MONDAY");
    assert!(val.is_none());
}

// GetValueFromFullName for "Season::WINTER" returns correct value.
#[test]
fn get_value_from_full_name_winter_found() {
    register_season_names();
    let result = TfEnum::get_value_from_full_name("Season::Winter");
    assert!(result.is_some());
    let (_tid, val) = result.unwrap();
    assert_eq!(val, Season::Winter as i32);
}

// Cross-type lookup: Season names are not found under Condiment.
#[test]
fn cross_type_lookup_fails() {
    register_condiment_names();
    register_season_names();
    let val = TfEnum::get_value_from_name::<Season>("Season", "Salt");
    assert!(val.is_none());
}

// IsKnownEnumType returns true for registered type names.
#[test]
fn is_known_enum_type_season_known() {
    register_season_names();
    assert!(TfEnum::is_known_enum_type("Season"));
}

// IsKnownEnumType returns false for names that were never registered.
#[test]
fn is_known_enum_type_sandwich_unknown() {
    assert!(!TfEnum::is_known_enum_type("Sandwich"));
}

// GetAllNames returns all registered names for the Condiment type.
#[test]
fn get_all_names_condiment() {
    register_condiment_names();
    let mut names = TfEnum::get_all_names::<Condiment>("Condiment");
    names.sort();
    assert!(names.contains(&"Ketchup".to_string()));
    assert!(names.contains(&"Pepper".to_string()));
    assert!(names.contains(&"Salt".to_string()));
}

// GetAllNames returns all registered names for Season.
#[test]
fn get_all_names_season() {
    register_season_names();
    let mut names = TfEnum::get_all_names::<Season>("Season");
    names.sort();
    assert!(names.contains(&"Autumn".to_string()));
    assert!(names.contains(&"Spring".to_string()));
    assert!(names.contains(&"Summer".to_string()));
    assert!(names.contains(&"Winter".to_string()));
}

// TfEnum wrapping Season::Summer passes IsA, GetValueAsInt, and GetValue checks.
#[test]
fn tf_enum_is_a_and_value() {
    let e = TfEnum::new(Season::Summer);
    assert!(e.is_a::<Season>());
    assert_eq!(e.value(), 3);
    assert_eq!(e.get::<Season>(), Season::Summer);
}

// Equality operators for TfEnum values of the same type.
#[test]
fn operators_equality() {
    assert_eq!(TfEnum::new(Season::Summer), TfEnum::new(Season::Summer));
    assert_ne!(TfEnum::new(Season::Summer), TfEnum::new(Season::Spring));
}

// Ordering operators for TfEnum values within the same type.
#[test]
fn operators_ordering() {
    let summer = TfEnum::new(Season::Summer);
    let spring = TfEnum::new(Season::Spring);

    assert!(summer > spring);
    assert!(summer >= spring);
    assert!(spring < summer);
    assert!(spring <= summer);
    assert!(summer >= summer);
    assert!(summer <= summer);
}
