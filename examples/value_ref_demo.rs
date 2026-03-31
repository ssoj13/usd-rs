//! Demo of ValueRef usage
//!
//! Run with: cargo run --example value_ref_demo

use usd::vt::{Value, ValueRef};

fn main() {
    println!("=== ValueRef Demo ===\n");

    // Create a Value
    let value = Value::from(42i32);
    println!("Created Value: {:?}", value);

    // Create a ValueRef from the Value
    let value_ref = ValueRef::from(&value);
    println!("Created ValueRef: {:?}", value_ref);

    // Check type
    println!("Is i32? {}", value_ref.is::<i32>());
    println!("Is f64? {}", value_ref.is::<f64>());

    // Get typed reference
    if let Some(&n) = value_ref.get::<i32>() {
        println!("Got value: {}", n);
    }

    // Create ValueRef from direct typed reference
    let number = 100i32;
    let direct_ref = ValueRef::from_typed(&number);
    println!("\nDirect ValueRef: {:?}", direct_ref);
    println!("Value: {:?}", direct_ref.get::<i32>());

    // Test with different types
    println!("\n=== Different Types ===");

    let float_val = Value::from(3.14f32);
    let float_ref = ValueRef::from(&float_val);
    println!("Float: {:?} = {:?}", float_ref, float_ref.get::<f32>());

    let string_val = Value::from(String::from("Hello, USD!"));
    let string_ref = ValueRef::from(&string_val);
    println!(
        "String: {:?} = {:?}",
        string_ref,
        string_ref.get::<String>()
    );

    let bool_val = Value::from(true);
    let bool_ref = ValueRef::from(&bool_val);
    println!("Bool: {:?} = {:?}", bool_ref, bool_ref.get::<bool>());

    // Test as_value roundtrip
    println!("\n=== Roundtrip Test ===");
    let original = Value::from(999i32);
    let ref_view = ValueRef::from(&original);
    let cloned = ref_view.as_value();
    println!("Original: {:?}", original);
    println!("Cloned: {:?}", cloned);
    println!("Equal? {}", original == cloned);

    // Test function parameter usage
    println!("\n=== Function Parameter ===");
    process_any_value(ValueRef::from(&value));
    process_any_value(ValueRef::from_typed(&number));
    process_any_value(ValueRef::from(&string_val));

    println!("\n=== Demo Complete ===");
}

fn process_any_value(val_ref: ValueRef) {
    if let Some(&n) = val_ref.get::<i32>() {
        println!("Processing integer: {}", n);
    } else if let Some(&f) = val_ref.get::<f32>() {
        println!("Processing float: {}", f);
    } else if let Some(s) = val_ref.get::<String>() {
        println!("Processing string: {}", s);
    } else {
        println!("Processing unknown type: {}", val_ref.type_name());
    }
}
