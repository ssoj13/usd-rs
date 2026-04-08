//! System message tokens for asynchronous processing.
//!
//! Corresponds to pxr/imaging/hd/systemMessages.h.
//! Defines messages used between scene indices and the application for
//! incremental/async work notification.

use usd_tf::Token;

/// Tokens for system messages used in scene index async processing.
///
/// Corresponds to C++ `HdSystemMessageTokens`.
pub struct HdSystemMessageTokens;

impl HdSystemMessageTokens {
    /// Indicates that asynchronous processing is allowed and to expect to
    /// receive "asyncPoll" messages to follow. This message provides no
    /// arguments.
    pub fn async_allow() -> Token {
        Token::new("asyncAllow")
    }

    /// Following a "asyncAllow" message, this will be called periodically on the
    /// application main (or rendering) thread to give scene indices an
    /// opportunity to send notices for completed asynchronous or incremental
    /// work.
    pub fn async_poll() -> Token {
        Token::new("asyncPoll")
    }
}

/// Pre-created tokens for common system messages.
pub mod tokens {
    use once_cell::sync::Lazy;
    use usd_tf::Token;

    /// asyncAllow - async processing allowed
    pub static ASYNC_ALLOW: Lazy<Token> = Lazy::new(|| Token::new("asyncAllow"));
    /// asyncPoll - poll for async completion
    pub static ASYNC_POLL: Lazy<Token> = Lazy::new(|| Token::new("asyncPoll"));
}
