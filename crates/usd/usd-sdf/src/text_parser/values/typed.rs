//! Typed value parsing for USDA format.
//!
//! This module handles complex typed values:
//! - **Time Samples**: `{ 0: value, 1: value, ... }` - animation keyframes
//! - **Splines**: `{ bezier, pre: held, 0: value, ... }` - animation curves
//! - **Array Edits**: `prepend`, `append`, `delete` operations
//!
//! # C++ Parity
//!
//! Matches the typed value rules from `textFileFormatParser.h`:
//! ```text
//! TimeSampleMap = { TimeSample, ... }           // lines 764-781
//! SplineValue = { SplineItem, ... }             // lines 783-981
//! ArrayEditValue = ArrayEditOp Value            // lines 549-623
//! ```
//!
//! # Time Samples
//!
//! Time samples are the primary animation data structure in USD:
//! ```text
//! {
//!     0: (0, 0, 0),
//!     24: (1, 0, 0),
//!     48: None,      // blocked sample
//! }
//! ```
//!
//! # Splines
//!
//! Splines provide smooth interpolation with tangent controls:
//! ```text
//! {
//!     bezier,
//!     pre: held,
//!     post: linear,
//!     0: 0.0,
//!     1: 1.0; pre (0.5); post curve (0.5),
//! }
//! ```

use crate::text_parser::error::{ParseErrorKind, ParseResult};
use crate::text_parser::tokens::{Keyword, TokenKind};
use crate::text_parser::value_context::{ArrayEdit, ArrayEditOp, Value};
use usd_vt::spline::{
    SplineCurveType, SplineExtrapolation, SplineInterpMode, SplineKnot, SplineLoopParams,
    SplineTangent, SplineTangentAlgorithm, SplineValue,
};

use super::ValueParser;

// ============================================================================
// Time Sample Types
// ============================================================================

/// A single time sample entry: time -> value.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeSample {
    /// The time code (frame number).
    pub time: f64,
    /// The value at this time (None if blocked).
    pub value: Option<Value>,
}

/// A map of time samples for animation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimeSampleMap {
    /// The samples ordered by time.
    pub samples: Vec<TimeSample>,
}

// ============================================================================
// Spline Types
// ============================================================================
//
// Spline types are now imported from usd_vt::spline
// This section kept for documentation purposes only

// ArrayEdit and ArrayEditOp are imported from value_context above

// ============================================================================
// Value Parser Implementation
// ============================================================================

impl<'a> ValueParser<'a> {
    /// Parses a time sample map: `{ time: value, ... }`.
    ///
    /// Time sample maps are used for animation in USD. Each entry maps
    /// a time code (frame) to a value. The special value `None` indicates
    /// a blocked (deleted) sample.
    ///
    /// # Grammar
    ///
    /// ```text
    /// TimeSampleMap = '{' TimeSample (',' TimeSample)* '}'
    /// TimeSample = Number ':' (None | TypedValue)
    /// ```
    ///
    /// # Examples
    ///
    /// ```text
    /// {
    ///     0: (0, 0, 0),
    ///     24: (1, 0, 0),
    ///     48: None,
    /// }
    /// ```
    pub fn parse_time_sample_map(&mut self) -> ParseResult<TimeSampleMap> {
        self.expect(&TokenKind::LeftBrace)?;

        let mut samples = Vec::new();

        // Parse entries until closing brace
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let sample = self.parse_time_sample()?;
            samples.push(sample);

            // Samples are separated by commas
            if self.match_kind(&TokenKind::Comma).is_none() {
                break;
            }
        }

        self.expect(&TokenKind::RightBrace)?;

        Ok(TimeSampleMap { samples })
    }

    /// Parses a single time sample entry: `time: value`.
    fn parse_time_sample(&mut self) -> ParseResult<TimeSample> {
        // Parse time (number)
        let time = self.parse_time_code()?;

        // Expect colon
        self.expect(&TokenKind::Colon)?;

        // Parse value (or None for blocked sample)
        let value =
            if self.check_keyword(Keyword::None) || self.check_keyword(Keyword::NoneLowercase) {
                self.advance();
                None
            } else {
                // Check for array edit operations
                if self.is_array_edit_start() {
                    let edit = self.parse_array_edit()?;
                    Some(Value::ArrayEdit(Box::new(edit)))
                } else {
                    Some(self.parse_value()?)
                }
            };

        Ok(TimeSample { time, value })
    }

    /// Parses a time code (frame number).
    fn parse_time_code(&mut self) -> ParseResult<f64> {
        let token = self
            .advance()
            .ok_or_else(|| self.error(ParseErrorKind::ExpectedNumber))?;

        match token.kind {
            TokenKind::Integer(n) => Ok(n as f64),
            TokenKind::Float(f) => Ok(f),
            _ => Err(self.error(ParseErrorKind::ExpectedNumber)),
        }
    }

    /// Checks if current token starts an array edit operation.
    fn is_array_edit_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(TokenKind::Keyword(Keyword::Prepend))
                | Some(TokenKind::Keyword(Keyword::Append))
                | Some(TokenKind::Keyword(Keyword::Delete))
                | Some(TokenKind::Keyword(Keyword::Add))
                | Some(TokenKind::Keyword(Keyword::Reorder))
                | Some(TokenKind::Keyword(Keyword::Write))
                | Some(TokenKind::Keyword(Keyword::Insert))
                | Some(TokenKind::Keyword(Keyword::Erase))
        )
    }

    /// Parses an array edit operation.
    ///
    /// Array edits allow modifying list opinions without full replacement:
    /// - `prepend value` - add to beginning
    /// - `append value` - add to end
    /// - `delete value` - remove matching items
    /// - `add value` - set union semantics
    /// - `reorder value` - reorder items
    /// - `write value to [index]` - write at index
    /// - `insert value at [index]` - insert at index
    /// - `erase [index]` - remove at index
    ///
    /// # Grammar
    ///
    /// ```text
    /// ArrayEditValue = ArrayEditOp Value | ArrayEditOp Value 'to'/'at' '[' Index ']'
    /// ArrayEditOp = 'prepend' | 'append' | 'delete' | 'add' | 'reorder' |
    ///               'write' | 'insert' | 'erase'
    /// ```
    pub fn parse_array_edit(&mut self) -> ParseResult<ArrayEdit> {
        let token = self
            .advance()
            .ok_or_else(|| self.error(ParseErrorKind::UnexpectedEof))?;

        let op = match &token.kind {
            TokenKind::Keyword(Keyword::Prepend) => ArrayEditOp::Prepend,
            TokenKind::Keyword(Keyword::Append) => ArrayEditOp::Append,
            TokenKind::Keyword(Keyword::Delete) => ArrayEditOp::Delete,
            TokenKind::Keyword(Keyword::Add) => ArrayEditOp::Add,
            TokenKind::Keyword(Keyword::Reorder) => ArrayEditOp::Reorder,
            TokenKind::Keyword(Keyword::Write) => ArrayEditOp::Write,
            TokenKind::Keyword(Keyword::Insert) => ArrayEditOp::Insert,
            TokenKind::Keyword(Keyword::Erase) => ArrayEditOp::Erase,
            _ => {
                return Err(self.error(ParseErrorKind::UnexpectedToken(
                    "expected array edit operation".to_string(),
                )));
            }
        };

        match op {
            // erase [index]
            ArrayEditOp::Erase => {
                let index = self.parse_array_edit_index()?;
                Ok(ArrayEdit {
                    op,
                    value: Box::new(Value::Int64(0)), // placeholder
                    index: Some(index),
                })
            }

            // write value to [index]
            ArrayEditOp::Write => {
                let value = self.parse_array_edit_value()?;
                self.expect_keyword(Keyword::To)?;
                let index = self.parse_array_edit_index()?;
                Ok(ArrayEdit {
                    op,
                    value: Box::new(value),
                    index: Some(index),
                })
            }

            // insert value at [index]
            ArrayEditOp::Insert => {
                let value = self.parse_array_edit_value()?;
                self.expect_keyword(Keyword::At)?;
                let index = self.parse_array_edit_index()?;
                Ok(ArrayEdit {
                    op,
                    value: Box::new(value),
                    index: Some(index),
                })
            }

            // prepend/append/delete/add/reorder value
            _ => {
                let value = self.parse_array_edit_value()?;
                Ok(ArrayEdit {
                    op,
                    value: Box::new(value),
                    index: None,
                })
            }
        }
    }

    /// Parses an array edit value (reference or literal).
    fn parse_array_edit_value(&mut self) -> ParseResult<Value> {
        // Check for reference: [index]
        if self.check(&TokenKind::LeftBracket) {
            let index = self.parse_array_edit_index()?;
            Ok(Value::Int64(index))
        } else {
            // Atomic value or tuple
            self.parse_value()
        }
    }

    /// Parses an array edit index: `[number]`.
    fn parse_array_edit_index(&mut self) -> ParseResult<i64> {
        self.expect(&TokenKind::LeftBracket)?;

        let token = self
            .advance()
            .ok_or_else(|| self.error(ParseErrorKind::ExpectedNumber))?;

        let index = match token.kind {
            TokenKind::Integer(n) => n,
            _ => return Err(self.error(ParseErrorKind::ExpectedNumber)),
        };

        self.expect(&TokenKind::RightBracket)?;
        Ok(index)
    }

    /// Parses a spline value.
    ///
    /// Splines provide smooth interpolation for animation with tangent
    /// controls, extrapolation modes, and looping.
    ///
    /// # Grammar
    ///
    /// ```text
    /// SplineValue = '{' SplineItem* '}'
    /// SplineItem = SplineCurveType | SplinePreExtrap | SplinePostExtrap |
    ///              SplineLoop | SplineKnot
    /// ```
    ///
    /// # Examples
    ///
    /// ```text
    /// {
    ///     bezier,
    ///     pre: held,
    ///     post: linear,
    ///     0: 0.0,
    ///     1: 1.0,
    /// }
    /// ```
    pub fn parse_spline_value(&mut self) -> ParseResult<SplineValue> {
        self.expect(&TokenKind::LeftBrace)?;

        let mut spline = SplineValue::default();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            self.parse_spline_item(&mut spline)?;

            // Items are separated by commas
            self.match_kind(&TokenKind::Comma);
        }

        self.expect(&TokenKind::RightBrace)?;

        Ok(spline)
    }

    /// Parses a single spline item.
    fn parse_spline_item(&mut self, spline: &mut SplineValue) -> ParseResult<()> {
        match self.peek_kind() {
            // Curve type: bezier or hermite
            Some(TokenKind::Keyword(Keyword::Bezier)) => {
                self.advance();
                spline.set_curve_type_value(SplineCurveType::Bezier);
            }
            Some(TokenKind::Keyword(Keyword::Hermite)) => {
                self.advance();
                spline.set_curve_type_value(SplineCurveType::Hermite);
            }

            // Pre-extrapolation: pre: mode
            Some(TokenKind::Keyword(Keyword::Pre)) => {
                self.advance();
                self.expect(&TokenKind::Colon)?;
                spline.set_pre_extrap(self.parse_spline_extrapolation()?);
            }

            // Post-extrapolation: post: mode
            Some(TokenKind::Keyword(Keyword::Post)) => {
                self.advance();
                self.expect(&TokenKind::Colon)?;
                spline.set_post_extrap(self.parse_spline_extrapolation()?);
            }

            // Loop parameters: loop: (start, end, pre, post, offset)
            Some(TokenKind::Keyword(Keyword::Loop)) => {
                spline.set_loop_params(Some(self.parse_spline_loop()?));
            }

            // Knot: time: value
            Some(TokenKind::Integer(_)) | Some(TokenKind::Float(_)) => {
                let knot = self.parse_spline_knot()?;
                spline.add_knot_parsed(knot);
            }

            Some(kind) => {
                return Err(self.error(ParseErrorKind::UnexpectedToken(format!(
                    "expected spline item, found {}",
                    kind
                ))));
            }

            None => {
                return Err(self.error(ParseErrorKind::UnexpectedEof));
            }
        }

        Ok(())
    }

    /// Parses a spline extrapolation mode.
    fn parse_spline_extrapolation(&mut self) -> ParseResult<SplineExtrapolation> {
        match self.peek_kind() {
            Some(TokenKind::Keyword(Keyword::None | Keyword::NoneLowercase)) => {
                self.advance();
                Ok(SplineExtrapolation::None)
            }
            Some(TokenKind::Keyword(Keyword::Held)) => {
                self.advance();
                Ok(SplineExtrapolation::Held)
            }
            Some(TokenKind::Keyword(Keyword::Linear)) => {
                self.advance();
                Ok(SplineExtrapolation::Linear)
            }
            Some(TokenKind::Keyword(Keyword::Sloped)) => {
                self.advance();
                self.expect(&TokenKind::LeftParen)?;
                let slope = self.parse_time_code()?;
                self.expect(&TokenKind::RightParen)?;
                Ok(SplineExtrapolation::Sloped(slope))
            }
            Some(TokenKind::Keyword(Keyword::Loop)) => {
                self.advance();
                // Check for loop mode
                match self.peek_kind() {
                    Some(TokenKind::Keyword(Keyword::Repeat)) => {
                        self.advance();
                        Ok(SplineExtrapolation::LoopRepeat)
                    }
                    Some(TokenKind::Keyword(Keyword::Reset)) => {
                        self.advance();
                        Ok(SplineExtrapolation::LoopReset)
                    }
                    Some(TokenKind::Keyword(Keyword::Oscillate)) => {
                        self.advance();
                        Ok(SplineExtrapolation::LoopOscillate)
                    }
                    _ => Ok(SplineExtrapolation::LoopRepeat), // default
                }
            }
            _ => Err(self.error(ParseErrorKind::UnexpectedToken(
                "expected extrapolation mode".to_string(),
            ))),
        }
    }

    /// Parses spline loop parameters.
    fn parse_spline_loop(&mut self) -> ParseResult<SplineLoopParams> {
        self.expect_keyword(Keyword::Loop)?;
        self.expect(&TokenKind::Colon)?;
        self.expect(&TokenKind::LeftParen)?;

        let proto_start = self.parse_time_code()?;
        self.expect(&TokenKind::Comma)?;

        let proto_end = self.parse_time_code()?;
        self.expect(&TokenKind::Comma)?;

        let num_pre_loops = self.parse_time_code()? as i64;
        self.expect(&TokenKind::Comma)?;

        let num_post_loops = self.parse_time_code()? as i64;
        self.expect(&TokenKind::Comma)?;

        let value_offset = self.parse_time_code()?;

        self.expect(&TokenKind::RightParen)?;

        Ok(SplineLoopParams {
            proto_start,
            proto_end,
            num_pre_loops,
            num_post_loops,
            value_offset,
        })
    }

    /// Parses a spline knot.
    fn parse_spline_knot(&mut self) -> ParseResult<SplineKnot> {
        let time = self.parse_time_code()?;
        self.expect(&TokenKind::Colon)?;

        // Parse value (may have pre-value with &)
        let (value, pre_value) = self.parse_spline_knot_value()?;

        // Parse optional knot parameters
        let mut pre_tangent = None;
        let mut post_interp = None;
        let mut post_tangent = None;
        let custom_data = None;

        // Check for semicolon followed by knot parameters
        if self.match_kind(&TokenKind::Semicolon).is_some() {
            while !self.check(&TokenKind::Comma) && !self.check(&TokenKind::RightBrace) {
                match self.peek_kind() {
                    Some(TokenKind::Keyword(Keyword::Pre)) => {
                        self.advance();
                        pre_tangent = Some(self.parse_spline_tangent()?);
                    }
                    Some(TokenKind::Keyword(Keyword::Post)) => {
                        self.advance();
                        post_interp = Some(self.parse_spline_interp_mode()?);
                        if self.check(&TokenKind::LeftParen) {
                            post_tangent = Some(self.parse_spline_tangent()?);
                        }
                    }
                    Some(TokenKind::LeftBrace) => {
                        // Note: Custom_data parsing requires Value type conversion.
                        // For now, skip custom_data parsing as it requires type conversion
                        let _dict_val = self.parse_dictionary_value()?;
                        // custom_data remains None until conversion is implemented
                    }
                    _ => break,
                }
                self.match_kind(&TokenKind::Semicolon);
            }
        }

        Ok(SplineKnot {
            time,
            value,
            pre_value,
            pre_tangent,
            post_interp,
            post_tangent,
            custom_data,
        })
    }

    /// Parses a spline knot value, optionally with pre-value.
    fn parse_spline_knot_value(&mut self) -> ParseResult<(f64, Option<f64>)> {
        let first = self.parse_time_code()?;

        // Check for pre-value separator (&)
        if self.check(&TokenKind::Ampersand) {
            self.advance();
            let second = self.parse_time_code()?;
            // first is pre-value, second is value
            Ok((second, Some(first)))
        } else {
            Ok((first, None))
        }
    }

    /// Parses a spline tangent.
    fn parse_spline_tangent(&mut self) -> ParseResult<SplineTangent> {
        self.expect(&TokenKind::LeftParen)?;

        let first = self.parse_time_code()?;

        // Check for additional components
        let (width, slope, algorithm) = if self.match_kind(&TokenKind::Comma).is_some() {
            let second = self.parse_time_code()?;

            if self.match_kind(&TokenKind::Comma).is_some() {
                // width, slope, algorithm
                let algo = self.parse_spline_tangent_algorithm()?;
                (Some(first), second, Some(algo))
            } else {
                // Could be width+slope or slope+algorithm
                // Try to peek for algorithm keyword
                if matches!(
                    self.peek_kind(),
                    Some(TokenKind::Keyword(Keyword::Custom))
                        | Some(TokenKind::Keyword(Keyword::AutoEase))
                ) {
                    // slope, algorithm
                    let algo = self.parse_spline_tangent_algorithm()?;
                    (None, first, Some(algo))
                } else {
                    // width, slope
                    (Some(first), second, None)
                }
            }
        } else {
            // Just slope
            (None, first, None)
        };

        self.expect(&TokenKind::RightParen)?;

        Ok(SplineTangent {
            width,
            slope,
            algorithm,
        })
    }

    /// Parses a spline tangent algorithm.
    fn parse_spline_tangent_algorithm(&mut self) -> ParseResult<SplineTangentAlgorithm> {
        match self.peek_kind() {
            Some(TokenKind::Keyword(Keyword::Custom)) => {
                self.advance();
                Ok(SplineTangentAlgorithm::Custom)
            }
            Some(TokenKind::Keyword(Keyword::AutoEase)) => {
                self.advance();
                Ok(SplineTangentAlgorithm::AutoEase)
            }
            _ => Err(self.error(ParseErrorKind::UnexpectedToken(
                "expected tangent algorithm".to_string(),
            ))),
        }
    }

    /// Parses a spline interpolation mode.
    fn parse_spline_interp_mode(&mut self) -> ParseResult<SplineInterpMode> {
        match self.peek_kind() {
            Some(TokenKind::Keyword(Keyword::None | Keyword::NoneLowercase)) => {
                self.advance();
                Ok(SplineInterpMode::None)
            }
            Some(TokenKind::Keyword(Keyword::Held)) => {
                self.advance();
                Ok(SplineInterpMode::Held)
            }
            Some(TokenKind::Keyword(Keyword::Linear)) => {
                self.advance();
                Ok(SplineInterpMode::Linear)
            }
            Some(TokenKind::Keyword(Keyword::Curve)) => {
                self.advance();
                Ok(SplineInterpMode::Curve)
            }
            _ => Err(self.error(ParseErrorKind::UnexpectedToken(
                "expected interpolation mode".to_string(),
            ))),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Time Sample Tests
    // ========================================================================

    #[test]
    fn test_parse_empty_time_samples() {
        let mut parser = ValueParser::new("{}");
        let result = parser.parse_time_sample_map().unwrap();
        assert!(result.samples.is_empty());
    }

    #[test]
    fn test_parse_single_time_sample() {
        let mut parser = ValueParser::new("{ 0: 42 }");
        let result = parser.parse_time_sample_map().unwrap();
        assert_eq!(result.samples.len(), 1);
        assert_eq!(result.samples[0].time, 0.0);
        assert!(matches!(result.samples[0].value, Some(Value::Int64(42))));
    }

    #[test]
    fn test_parse_multiple_time_samples() {
        let mut parser = ValueParser::new("{ 0: 1, 24: 2, 48: 3 }");
        let result = parser.parse_time_sample_map().unwrap();
        assert_eq!(result.samples.len(), 3);
        assert_eq!(result.samples[0].time, 0.0);
        assert_eq!(result.samples[1].time, 24.0);
        assert_eq!(result.samples[2].time, 48.0);
    }

    #[test]
    fn test_parse_time_sample_with_tuple() {
        let mut parser = ValueParser::new("{ 0: (1, 2, 3) }");
        let result = parser.parse_time_sample_map().unwrap();
        assert_eq!(result.samples.len(), 1);
        assert!(matches!(result.samples[0].value, Some(Value::Tuple(_))));
    }

    #[test]
    fn test_parse_time_sample_with_none() {
        let mut parser = ValueParser::new("{ 0: None }");
        let result = parser.parse_time_sample_map().unwrap();
        assert_eq!(result.samples.len(), 1);
        assert!(result.samples[0].value.is_none());
    }

    #[test]
    fn test_parse_time_sample_float_time() {
        let mut parser = ValueParser::new("{ 0.5: 1, 1.5: 2 }");
        let result = parser.parse_time_sample_map().unwrap();
        assert_eq!(result.samples.len(), 2);
        assert!((result.samples[0].time - 0.5).abs() < 0.001);
        assert!((result.samples[1].time - 1.5).abs() < 0.001);
    }

    // ========================================================================
    // Array Edit Tests
    // ========================================================================

    #[test]
    fn test_parse_prepend() {
        let mut parser = ValueParser::new("prepend 42");
        let result = parser.parse_array_edit().unwrap();
        assert_eq!(result.op, ArrayEditOp::Prepend);
        assert!(matches!(*result.value, Value::Int64(42)));
        assert!(result.index.is_none());
    }

    #[test]
    fn test_parse_append() {
        let mut parser = ValueParser::new("append (1, 2, 3)");
        let result = parser.parse_array_edit().unwrap();
        assert_eq!(result.op, ArrayEditOp::Append);
        assert!(matches!(*result.value, Value::Tuple(_)));
    }

    #[test]
    fn test_parse_delete() {
        let mut parser = ValueParser::new("delete [0]");
        let result = parser.parse_array_edit().unwrap();
        assert_eq!(result.op, ArrayEditOp::Delete);
        assert!(matches!(*result.value, Value::Int64(0)));
    }

    #[test]
    fn test_parse_write_to() {
        let mut parser = ValueParser::new("write 42 to [5]");
        let result = parser.parse_array_edit().unwrap();
        assert_eq!(result.op, ArrayEditOp::Write);
        assert!(matches!(*result.value, Value::Int64(42)));
        assert_eq!(result.index, Some(5));
    }

    #[test]
    fn test_parse_insert_at() {
        let mut parser = ValueParser::new("insert (1, 2) at [3]");
        let result = parser.parse_array_edit().unwrap();
        assert_eq!(result.op, ArrayEditOp::Insert);
        assert!(matches!(*result.value, Value::Tuple(_)));
        assert_eq!(result.index, Some(3));
    }

    #[test]
    fn test_parse_erase() {
        let mut parser = ValueParser::new("erase [7]");
        let result = parser.parse_array_edit().unwrap();
        assert_eq!(result.op, ArrayEditOp::Erase);
        assert_eq!(result.index, Some(7));
    }

    // ========================================================================
    // Spline Tests
    // ========================================================================

    #[test]
    fn test_parse_empty_spline() {
        let mut parser = ValueParser::new("{}");
        let result = parser.parse_spline_value().unwrap();
        assert!(result.knots().is_empty());
    }

    #[test]
    fn test_parse_spline_curve_type() {
        let mut parser = ValueParser::new("{ bezier }");
        let result = parser.parse_spline_value().unwrap();
        assert_eq!(result.curve_type(), SplineCurveType::Bezier);

        let mut parser = ValueParser::new("{ hermite }");
        let result = parser.parse_spline_value().unwrap();
        assert_eq!(result.curve_type(), SplineCurveType::Hermite);
    }

    #[test]
    fn test_parse_spline_extrapolation() {
        let mut parser = ValueParser::new("{ pre: held, post: linear }");
        let result = parser.parse_spline_value().unwrap();
        assert_eq!(result.pre_extrapolation(), &SplineExtrapolation::Held);
        assert_eq!(result.post_extrapolation(), &SplineExtrapolation::Linear);
    }

    #[test]
    fn test_parse_spline_sloped_extrapolation() {
        let mut parser = ValueParser::new("{ pre: sloped(0.5) }");
        let result = parser.parse_spline_value().unwrap();
        match result.pre_extrapolation() {
            SplineExtrapolation::Sloped(slope) => {
                assert!((slope - 0.5).abs() < 0.001);
            }
            _ => panic!("expected sloped extrapolation"),
        }
    }

    #[test]
    fn test_parse_spline_simple_knots() {
        let mut parser = ValueParser::new("{ 0: 0.0, 1: 1.0 }");
        let result = parser.parse_spline_value().unwrap();
        assert_eq!(result.knots().len(), 2);
        assert_eq!(result.knots()[0].time, 0.0);
        assert_eq!(result.knots()[0].value, 0.0);
        assert_eq!(result.knots()[1].time, 1.0);
        assert_eq!(result.knots()[1].value, 1.0);
    }

    #[test]
    fn test_parse_spline_dual_value() {
        let mut parser = ValueParser::new("{ 0: 0.5 & 1.0 }");
        let result = parser.parse_spline_value().unwrap();
        assert_eq!(result.knots().len(), 1);
        assert_eq!(result.knots()[0].value, 1.0);
        assert_eq!(result.knots()[0].pre_value, Some(0.5));
    }

    #[test]
    fn test_parse_complete_spline() {
        let mut parser = ValueParser::new(
            r#"{
            bezier,
            pre: held,
            post: linear,
            0: 0.0,
            24: 1.0,
            48: 0.0
        }"#,
        );
        let result = parser.parse_spline_value().unwrap();
        assert_eq!(result.curve_type(), SplineCurveType::Bezier);
        assert_eq!(result.pre_extrapolation(), &SplineExtrapolation::Held);
        assert_eq!(result.post_extrapolation(), &SplineExtrapolation::Linear);
        assert_eq!(result.knots().len(), 3);
    }
}
