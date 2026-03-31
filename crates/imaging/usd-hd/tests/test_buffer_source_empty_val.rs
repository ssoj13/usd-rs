// Port of pxr/imaging/hd/testenv/testHdBufferSourceEmptyVal.cpp

use usd_hd::resource::HdBufferSource;
use usd_hd::vt_buffer_source::HdVtBufferSource;
use usd_tf::Token;
use usd_vt::Value;

#[test]
fn test_buffer_source_empty_val() {
    let empty_value = Value::default();
    let buffer_source = HdVtBufferSource::new(Token::new("points"), empty_value, 1, true);

    // A buffer source constructed from an empty value should be invalid
    assert!(
        !buffer_source.is_valid(),
        "Buffer source from empty value should be Invalid"
    );
}
