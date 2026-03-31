use usd_tf::{Token, to_string_vector, to_token_vector};

// Empty tokens are equal and hash identically.
#[test]
fn empty_tokens_equal() {
    let empty1 = Token::empty();
    let empty2 = Token::empty();
    assert_eq!(empty1, empty2);
    assert_eq!(empty1.hash(), empty2.hash());
}

// Empty token equals empty string literal and String.
#[test]
fn empty_token_eq_empty_string() {
    let empty1 = Token::empty();
    let empty2 = Token::empty();
    let s_empty = String::new();

    assert_eq!(empty1, "");
    assert_eq!(empty2, "");
    assert_eq!(empty1, s_empty);
    assert_eq!(empty2, s_empty);
}

// Non-empty token differs from empty token.
#[test]
fn non_empty_differs_from_empty() {
    let empty = Token::empty();
    let non_empty = Token::new("nonEmpty");

    assert_ne!(empty, non_empty);
    assert!(empty.is_empty());
    assert!(!non_empty.is_empty());
}

// Comparison between non-empty token and empty string.
#[test]
fn non_empty_ne_empty_string() {
    let non_empty = Token::new("nonEmpty");
    let s_empty = String::new();

    assert_ne!(s_empty, non_empty.as_str());
    assert_ne!(non_empty, "");
}

// Token::swap() exchanges two tokens.
#[test]
fn token_swap() {
    let mut t_empty = Token::empty();
    let mut t_non_empty = Token::new("nonEmpty");

    t_empty.swap(&mut t_non_empty);

    assert!(t_non_empty.is_empty());
    assert!(!t_empty.is_empty());
    assert_eq!(t_empty, "nonEmpty");
    assert_eq!(t_non_empty, "");
}

// std::mem::swap exchanges tokens correctly.
// C++: std::swap(nonEmpty3, empty1) — maps to swap(t_non_empty, t_empty).
// After swap: t_non_empty holds "" (old empty), t_empty holds "nonEmpty".
#[test]
fn std_swap_tokens() {
    let mut t_empty = Token::empty();
    let mut t_non_empty = Token::new("nonEmpty");

    // Mirrors: std::swap(nonEmpty3, empty1)
    std::mem::swap(&mut t_non_empty, &mut t_empty);

    assert!(t_non_empty.is_empty());
    assert!(!t_empty.is_empty());
    assert_eq!(t_non_empty, "");
    assert_eq!(t_empty, "nonEmpty");
}

// Tokens created from the same string are equal and share the same hash.
#[test]
fn same_string_tokens_equal() {
    let a1 = "alphabet".to_string();
    let a2 = "alphabet";

    let b1 = "barnacle".to_string();
    let b2 = "barnacle";

    let c1 = "cinnamon".to_string();
    let c2 = "cinnamon";

    assert_eq!(Token::new(&a1), Token::new(a2));
    assert_eq!(Token::new(&a1).hash(), Token::new(a2).hash());

    assert_eq!(Token::new(&b1), Token::new(b2));
    assert_eq!(Token::new(&b1).hash(), Token::new(b2).hash());

    assert_eq!(Token::new(&c1), Token::new(c2));
    assert_eq!(Token::new(&c1).hash(), Token::new(c2).hash());
}

// Tokens from different strings are not equal and have different hashes.
#[test]
fn different_string_tokens_not_equal() {
    let a = Token::new("alphabet");
    let b = Token::new("barnacle");
    let c = Token::new("cinnamon");

    assert_ne!(a.hash(), b.hash());

    assert_ne!(a, b);
    assert_ne!(a, c);
    assert_ne!(b, c);
}

// Lexicographic ordering: "alphabet" < "barnacle".
#[test]
fn token_ordering() {
    let a = Token::new("alphabet");
    let b = Token::new("barnacle");

    assert!(a < b);
    assert!(b > a);
}

// Copy construction and assignment round-trips.
#[test]
fn copy_and_assign() {
    let a = Token::new("alphabet");
    let b = Token::new("barnacle");

    let t1 = Token::new("alphabet");
    let t2 = t1.clone(); // copy construct
    assert_eq!(t1, t2);

    let t1 = Token::new("barnacle"); // reassign
    assert_ne!(t1, t2);

    let t2 = Token::new("barnacle"); // reassign from &str
    assert_eq!(t1, t2);
    assert_eq!(t1, Token::new("barnacle"));

    // Sanity: originals still correct
    let _ = a;
    let _ = b;
}

// to_token_vector converts String slices to tokens correctly.
#[test]
fn to_token_vector_roundtrip() {
    let strings = vec![
        "string1".to_string(),
        "string2".to_string(),
        "string3".to_string(),
    ];

    let tokens = to_token_vector(&strings);

    assert_eq!(Token::new(&strings[0]), tokens[0]);
    assert_eq!(Token::new(&strings[1]), tokens[1]);
    assert_eq!(Token::new(&strings[2]), tokens[2]);
}

// to_string_vector converts tokens back to strings correctly.
#[test]
fn to_string_vector_roundtrip() {
    let strings = vec![
        "string1".to_string(),
        "string2".to_string(),
        "string3".to_string(),
    ];

    let tokens = to_token_vector(&strings);
    let strings2 = to_string_vector(&tokens);

    assert_eq!(Token::new(&strings2[0]), tokens[0]);
    assert_eq!(Token::new(&strings2[1]), tokens[1]);
    assert_eq!(Token::new(&strings2[2]), tokens[2]);
}
