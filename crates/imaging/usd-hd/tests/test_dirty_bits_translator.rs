// Port of pxr/imaging/hd/testenv/testHdDirtyBitsTranslator.cpp

use usd_hd::change_tracker::HdRprimDirtyBits;
use usd_hd::data_source::{HdDataSourceLocator, HdDataSourceLocatorSet};
use usd_hd::dirty_bits_translator::{HD_CLEAN, HdDirtyBits, HdDirtyBitsTranslator};
use usd_hd::schema::{HdPrimvarsSchema, PRIMVAR_VALUE};
use usd_tf::Token;

const DIRTY_PROTEIN: HdDirtyBits = 1 << 0;
const DIRTY_TORTILLA: HdDirtyBits = 1 << 1;
const DIRTY_SALSA: HdDirtyBits = 1 << 2;

fn taco_token() -> Token {
    Token::new("taco")
}
fn burger_token() -> Token {
    Token::new("burger")
}
fn protein_token() -> Token {
    Token::new("protein")
}
fn tortilla_token() -> Token {
    Token::new("tortilla")
}
fn salsa_token() -> Token {
    Token::new("salsa")
}

fn locator_set_to_dirty_bits_for_tacos(set: &HdDataSourceLocatorSet, bits: &mut HdDirtyBits) {
    if set.intersects_locator(&HdDataSourceLocator::from_tokens_2(
        taco_token(),
        protein_token(),
    )) {
        *bits |= DIRTY_PROTEIN;
    }
    if set.intersects_locator(&HdDataSourceLocator::from_tokens_2(
        taco_token(),
        tortilla_token(),
    )) {
        *bits |= DIRTY_TORTILLA;
    }
    if set.intersects_locator(&HdDataSourceLocator::from_tokens_2(
        taco_token(),
        salsa_token(),
    )) {
        *bits |= DIRTY_SALSA;
    }
}

fn dirty_bits_to_locator_set_for_tacos(bits: HdDirtyBits, set: &mut HdDataSourceLocatorSet) {
    if bits & DIRTY_PROTEIN != 0 {
        set.insert(HdDataSourceLocator::from_tokens_2(
            taco_token(),
            protein_token(),
        ));
    }
    if bits & DIRTY_TORTILLA != 0 {
        set.insert(HdDataSourceLocator::from_tokens_2(
            taco_token(),
            tortilla_token(),
        ));
    }
    if bits & DIRTY_SALSA != 0 {
        set.insert(HdDataSourceLocator::from_tokens_2(
            taco_token(),
            salsa_token(),
        ));
    }
}

#[test]
fn test_custom_sprim_types() {
    HdDirtyBitsTranslator::register_translators_for_custom_sprim_type(
        taco_token(),
        Box::new(locator_set_to_dirty_bits_for_tacos),
        Box::new(dirty_bits_to_locator_set_for_tacos),
    );

    // Dirtying an unrelated locator should not dirty a taco
    let dirty_stuff =
        HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::from_token(Token::new("camera")));

    assert_eq!(
        HdDirtyBitsTranslator::sprim_locator_set_to_dirty_bits(&taco_token(), &dirty_stuff),
        HD_CLEAN,
        "Expected clean taco"
    );

    // Unknown burger type should be AllDirty
    assert_ne!(
        HdDirtyBitsTranslator::sprim_locator_set_to_dirty_bits(&burger_token(), &dirty_stuff),
        HD_CLEAN,
        "Expected dirty burger"
    );

    // Round-trip of bits
    let bits: HdDirtyBits = DIRTY_TORTILLA | DIRTY_PROTEIN;
    let mut set = HdDataSourceLocatorSet::new();
    HdDirtyBitsTranslator::sprim_dirty_bits_to_locator_set(&taco_token(), bits, &mut set);

    assert_eq!(
        HdDirtyBitsTranslator::sprim_locator_set_to_dirty_bits(&taco_token(), &set),
        bits,
        "Roundtrip of dirty taco doesn't match"
    );
}

#[test]
fn test_custom_rprim_types() {
    HdDirtyBitsTranslator::register_translators_for_custom_rprim_type(
        taco_token(),
        Box::new(locator_set_to_dirty_bits_for_tacos),
        Box::new(dirty_bits_to_locator_set_for_tacos),
    );

    // Dirtying an unrelated locator should not dirty a taco
    let dirty_stuff =
        HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::from_token(Token::new("camera")));

    assert_eq!(
        HdDirtyBitsTranslator::rprim_locator_set_to_dirty_bits(&taco_token(), &dirty_stuff),
        HD_CLEAN,
        "Expected clean taco"
    );

    // Round-trip of bits
    let bits: HdDirtyBits = DIRTY_TORTILLA | DIRTY_PROTEIN;
    let mut set = HdDataSourceLocatorSet::new();
    HdDirtyBitsTranslator::rprim_dirty_bits_to_locator_set(&taco_token(), bits, &mut set);

    assert_eq!(
        HdDirtyBitsTranslator::rprim_locator_set_to_dirty_bits(&taco_token(), &set),
        bits,
        "Roundtrip of dirty taco doesn't match"
    );
}

#[test]
fn test_mesh_points_primvar_value_locator_maps_to_dirty_points() {
    let set = HdDataSourceLocatorSet::from_locator(
        HdPrimvarsSchema::get_points_locator().append(&PRIMVAR_VALUE),
    );
    let bits = HdDirtyBitsTranslator::rprim_locator_set_to_dirty_bits(&Token::new("mesh"), &set);
    assert_ne!(
        bits & HdRprimDirtyBits::DIRTY_POINTS,
        0,
        "mesh points primvarValue locator should dirty DIRTY_POINTS"
    );
}
