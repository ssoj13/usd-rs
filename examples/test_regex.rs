// Example testing ArchRegex

use usd::arch::ArchRegex;

fn main() {
    println!("Testing ArchRegex...\n");

    // Test 1: Simple pattern
    println!("1. Simple pattern:");
    let re = ArchRegex::new("hello", 0).unwrap();
    assert!(re.is_valid());
    assert!(re.match_str("hello world"));
    assert!(!re.match_str("goodbye"));
    println!("   ✓ Works");

    // Test 2: Case insensitive
    println!("2. Case insensitive:");
    let re = ArchRegex::new("HELLO", ArchRegex::CASE_INSENSITIVE).unwrap();
    assert!(re.match_str("hello"));
    assert!(re.match_str("HELLO"));
    assert!(re.match_str("HeLLo"));
    println!("   ✓ Works");

    // Test 3: Glob pattern
    println!("3. Glob pattern:");
    let re = ArchRegex::new("*.txt", ArchRegex::GLOB).unwrap();
    assert!(re.match_str("file.txt"));
    assert!(re.match_str("test.txt"));
    assert!(!re.match_str("file.rs"));
    println!("   ✓ Works");

    // Test 4: Glob with ?
    println!("4. Glob with ? wildcard:");
    let re = ArchRegex::new("file?.rs", ArchRegex::GLOB).unwrap();
    assert!(re.match_str("file1.rs"));
    assert!(re.match_str("fileA.rs"));
    assert!(!re.match_str("file.rs"));
    assert!(!re.match_str("file12.rs"));
    println!("   ✓ Works");

    // Test 5: Invalid pattern
    println!("5. Error handling:");
    let re = ArchRegex::from_pattern("(unclosed", 0);
    assert!(!re.is_valid());
    assert!(!re.get_error().is_empty());
    println!("   Error: {}", re.get_error());
    println!("   ✓ Works");

    // Test 6: Newline handling
    println!("6. Newline handling:");
    let re = ArchRegex::new("hello.*world", 0).unwrap();
    assert!(re.match_str("hello\nworld"));
    assert!(re.match_str("hello beautiful\nworld"));
    println!("   ✓ Works");

    // Test 7: Clone
    println!("7. Clone:");
    let re1 = ArchRegex::new("test", 0).unwrap();
    let re2 = re1.clone();
    assert!(re2.is_valid());
    assert!(re2.match_str("test"));
    println!("   ✓ Works");

    println!("\n✅ All tests passed!");
}
