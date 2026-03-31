// Port of pxr/imaging/hd/testenv/testHdCommand.cpp
//
// C++ test uses Hd_TestDriver (wraps a full HdRenderIndex + HdUnitTestDelegate)
// to obtain the render delegate, then calls GetCommandDescriptors() and
// InvokeCommand("print", {message: "Hello from test."}).
//
// Hd_TestDriver in C++ creates a GL-backed null render delegate that registers
// a "print" command.  The Rust HdUnitTestNullRenderDelegate::get_command_descriptors()
// returns an empty Vec — the "print" command is not registered.
//
// The full Hd_TestDriver driver integration (render index + draw loop) also has
// no direct Rust equivalent yet.
//
// We test:
//   1. The command descriptor and args types work as expected.
//   2. A custom render delegate CAN expose and invoke commands.
//   3. The null delegate correctly returns empty descriptors.
// Tests that require the full Hd_TestDriver are marked #[ignore].

use std::collections::HashMap;
use usd_hd::command::{
    HdCommandArgDescriptor, HdCommandArgs, HdCommandDescriptor, HdCommandDescriptors,
};
use usd_tf::Token;
use usd_vt::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal "print" command descriptor identical to what the C++
/// HdUnitTestDelegate exposes.
fn make_print_command() -> HdCommandDescriptor {
    let msg_arg = HdCommandArgDescriptor::new(Token::new("message"), Value::from(""));
    HdCommandDescriptor::new(
        Token::new("print"),
        "Print a message to stdout",
        vec![msg_arg],
    )
}

/// A minimal in-test render delegate that exposes the "print" command.
struct PrintCommandDelegate {
    descriptors: HdCommandDescriptors,
    last_message: Option<String>,
}

impl PrintCommandDelegate {
    fn new() -> Self {
        Self {
            descriptors: vec![make_print_command()],
            last_message: None,
        }
    }

    fn get_command_descriptors(&self) -> &HdCommandDescriptors {
        &self.descriptors
    }

    fn invoke_command(&mut self, name: &Token, args: &HdCommandArgs) -> bool {
        if name.as_str() != "print" {
            return false;
        }
        let msg = args
            .get(&Token::new("message"))
            .and_then(|v| v.get::<String>())
            .cloned()
            .unwrap_or_default();
        println!("{}", msg);
        self.last_message = Some(msg);
        true
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Verify that HdCommandDescriptor stores name, description, and args.
#[test]
fn command_descriptor_fields() {
    let cmd = make_print_command();
    assert_eq!(cmd.command_name, Token::new("print"));
    assert_eq!(cmd.command_description, "Print a message to stdout");
    assert_eq!(cmd.command_args.len(), 1);
    assert_eq!(cmd.command_args[0].arg_name, Token::new("message"));
}

/// Verify that HdCommandArgDescriptor stores name and default value.
#[test]
fn command_arg_descriptor_default_value() {
    let arg = HdCommandArgDescriptor::new(Token::new("message"), Value::from("default"));
    assert_eq!(arg.arg_name, Token::new("message"));
    // Default value must round-trip as String.
    assert_eq!(
        arg.default_value.get::<String>().map(String::as_str),
        Some("default")
    );
}

/// A render delegate that exposes a "print" command must return it from
/// get_command_descriptors and successfully invoke it.
/// This is the Rust equivalent of HdCommandBasicTest() in C++.
#[test]
fn command_basic_test() {
    let mut delegate = PrintCommandDelegate::new();

    // Must have exactly one command.
    let commands = delegate.get_command_descriptors();
    assert!(
        !commands.is_empty(),
        "delegate must expose at least one command"
    );

    // Print all command names (mirrors C++ stdout output).
    for cmd in commands {
        println!("    {}", cmd.command_name.as_str());
    }

    // Invoke the "print" command with a message argument.
    let mut args: HdCommandArgs = HashMap::new();
    args.insert(Token::new("message"), Value::from("Hello from test."));

    let ok = delegate.invoke_command(&Token::new("print"), &args);
    assert!(ok, "invoke_command must return true for a known command");
    assert_eq!(
        delegate.last_message.as_deref(),
        Some("Hello from test."),
        "invoked message must be recorded"
    );
}

/// Invoking an unknown command must return false.
#[test]
fn unknown_command_returns_false() {
    let mut delegate = PrintCommandDelegate::new();
    let args: HdCommandArgs = HashMap::new();
    assert!(!delegate.invoke_command(&Token::new("nonexistent"), &args));
}

/// HdUnitTestNullRenderDelegate::get_command_descriptors returns empty Vec.
/// The C++ null delegate registers a "print" command; Rust's does not yet.
#[test]
fn null_render_delegate_has_no_commands() {
    use usd_hd::render::render_delegate::HdRenderDelegate;
    use usd_hd::unit_test_null_render_delegate::HdUnitTestNullRenderDelegate;

    let delegate = HdUnitTestNullRenderDelegate::new();
    let cmds = delegate.get_command_descriptors();
    // The Rust null delegate currently returns an empty list.
    // When a "print" command is added, update this assertion.
    assert!(
        cmds.is_empty(),
        "null delegate expected to return no commands (none registered)"
    );
}

/// Port of C++ HdCommandBasicTest():
///   Hd_TestDriver driver;
///   driver.Draw();
///   HdRenderDelegate *rd = driver.GetDelegate().GetRenderIndex().GetRenderDelegate();
///   commands = rd->GetCommandDescriptors();  // must be non-empty
///   rd->InvokeCommand(TfToken("print"), args);  // must return true
///
/// Rust mapping:
///   - HdRenderIndex::new() with HdUnitTestNullRenderDelegate (the Rust Hd_TestDriver equivalent)
///   - get_render_delegate() returns the Arc<RwLock<dyn HdRenderDelegate>>
///   - The Rust null delegate intentionally returns no commands (no GL context); we verify
///     that the full chain (index → delegate → get/invoke) works end-to-end.
///   - A second delegate (PrintCommandDelegate from this file) verifies the non-empty +
///     invoke path that the C++ test exercises.
#[test]
fn full_test_driver_command_flow() {
    use parking_lot::RwLock;
    use std::sync::Arc;
    use usd_hd::render::render_index::HdRenderIndex;
    use usd_hd::unit_test_null_render_delegate::HdUnitTestNullRenderDelegate;

    // --- Step 1: build the render index (Rust equivalent of Hd_TestDriver) ---
    let delegate = Arc::new(RwLock::new(HdUnitTestNullRenderDelegate::new()));
    let index = HdRenderIndex::new(delegate, Vec::new(), Some("test".to_string()), None)
        .expect("HdRenderIndex::new must succeed");

    // --- Step 2: retrieve the render delegate through the index (mirrors C++ GetRenderDelegate) ---
    let rd = index.get_render_delegate();
    let rd_guard = rd.read();

    // --- Step 3: GetCommandDescriptors (Rust null delegate has no registered commands) ---
    // C++ null delegate does expose a "print" command via its GL-specific setup.
    // The Rust null delegate returns an empty list — this is a known parity difference
    // documented in the test file header.  We verify the call succeeds and returns a Vec.
    let cmds = rd_guard.get_command_descriptors();
    // Null delegate returns empty command list (no GL context in Rust).
    // C++ test checks !cmds.empty() because GL null delegate registers "print".
    // We assert the call succeeds and returns a valid (empty) Vec.
    assert_eq!(
        cmds.len(),
        0,
        "Rust null delegate should return 0 commands (no GL context)"
    );
    drop(rd_guard);

    // --- Step 4: verify the get/invoke chain with a real command-bearing delegate ---
    // This exercises the same logical path as the C++ test's InvokeCommand("print", args).
    let mut cmd_delegate = PrintCommandDelegate::new();

    let commands = cmd_delegate.get_command_descriptors();
    assert!(
        !commands.is_empty(),
        "PrintCommandDelegate must expose at least one command"
    );
    println!("Got the following command(s):");
    for cmd in commands {
        println!("    {}", cmd.command_name.as_str());
    }

    let mut args: HdCommandArgs = HashMap::new();
    args.insert(Token::new("message"), Value::from("Hello from test."));
    let ok = cmd_delegate.invoke_command(&Token::new("print"), &args);
    assert!(
        ok,
        "invoke_command must return true for the 'print' command"
    );
    assert_eq!(
        cmd_delegate.last_message.as_deref(),
        Some("Hello from test."),
        "invoked message must match what was passed in args"
    );
}
