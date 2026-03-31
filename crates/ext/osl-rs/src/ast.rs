//! AST — Abstract Syntax Tree for OSL.
//!
//! Port of `ast.h` from the OSL compiler. Defines all AST node types
//! produced by the parser and consumed by type checking and code generation.

use crate::lexer::SourceLoc;
use crate::typespec::TypeSpec;

/// Operator types for binary, unary, and assignment expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    // Assignment operators
    Assign,       // =
    AddAssign,    // +=
    SubAssign,    // -=
    MulAssign,    // *=
    DivAssign,    // /=
    BitAndAssign, // &=
    BitOrAssign,  // |=
    BitXorAssign, // ^=
    ShlAssign,    // <<=
    ShrAssign,    // >>=

    // Binary operators
    Add,       // +
    Sub,       // -
    Mul,       // *
    Div,       // /
    Mod,       // %
    Eq,        // ==
    NotEq,     // !=
    Less,      // <
    Greater,   // >
    LessEq,    // <=
    GreaterEq, // >=
    LogAnd,    // &&
    LogOr,     // ||
    BitAnd,    // &
    BitOr,     // |
    BitXor,    // ^
    Shl,       // <<
    Shr,       // >>

    // Unary operators
    Neg,     // - (unary)
    Not,     // !
    BitNot,  // ~
    PreInc,  // ++ (prefix)
    PreDec,  // -- (prefix)
    PostInc, // ++ (postfix)
    PostDec, // -- (postfix)
}

/// Loop type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopType {
    For,
    While,
    DoWhile,
}

/// Loop modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMod {
    Break,
    Continue,
}

/// Literal value kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    Int(i32),
    Float(f32),
    String(String),
}

impl LiteralValue {
    /// Negate a numeric literal in-place (C++ ast.h:1056-1060 ASTliteral::negate).
    pub fn negate(&mut self) {
        match self {
            LiteralValue::Int(v) => *v = -*v,
            LiteralValue::Float(v) => *v = -*v,
            LiteralValue::String(_) => {} // no-op for strings
        }
    }
}

/// A unique AST node ID for tracking.
pub type NodeId = u32;

/// Core AST node.
#[derive(Debug, Clone)]
pub struct ASTNode {
    /// Unique node ID.
    pub id: NodeId,
    /// The specific node variant.
    pub kind: ASTNodeKind,
    /// Source location.
    pub loc: SourceLoc,
    /// Resolved type (filled during type-checking).
    pub typespec: TypeSpec,
}

impl ASTNode {
    pub fn new(id: NodeId, kind: ASTNodeKind, loc: SourceLoc) -> Self {
        Self {
            id,
            kind,
            loc,
            typespec: TypeSpec::UNKNOWN,
        }
    }
}

/// All AST node variants.
#[derive(Debug, Clone)]
pub enum ASTNodeKind {
    // ----- Top-level declarations -----
    ShaderDeclaration {
        shader_type: String,
        name: String,
        formals: Vec<Box<ASTNode>>,
        statements: Vec<Box<ASTNode>>,
        metadata: Vec<Box<ASTNode>>,
    },

    FunctionDeclaration {
        name: String,
        return_type: TypeSpec,
        formals: Vec<Box<ASTNode>>,
        statements: Vec<Box<ASTNode>>,
        metadata: Vec<Box<ASTNode>>,
        is_builtin: bool,
    },

    // ----- Variable declarations -----
    VariableDeclaration {
        name: String,
        typespec: TypeSpec,
        init: Option<Box<ASTNode>>,
        is_param: bool,
        is_output: bool,
        is_metadata: bool,
        metadata: Vec<Box<ASTNode>>,
    },

    CompoundInitializer {
        elements: Vec<Box<ASTNode>>,
        /// Whether this initializer can be used as a type constructor
        /// (C++ ast.h:778 m_ctor / canconstruct flag).
        canconstruct: bool,
    },

    // ----- Expressions -----
    VariableRef {
        name: String,
    },

    PreIncDec {
        op: Operator,
        expr: Box<ASTNode>,
    },

    PostIncDec {
        op: Operator,
        expr: Box<ASTNode>,
    },

    Index {
        base: Box<ASTNode>,
        index: Box<ASTNode>,
        index2: Option<Box<ASTNode>>,
        index3: Option<Box<ASTNode>>,
    },

    StructSelect {
        base: Box<ASTNode>,
        field: String,
    },

    BinaryExpression {
        op: Operator,
        left: Box<ASTNode>,
        right: Box<ASTNode>,
    },

    UnaryExpression {
        op: Operator,
        expr: Box<ASTNode>,
    },

    AssignExpression {
        op: Operator,
        lvalue: Box<ASTNode>,
        expr: Box<ASTNode>,
    },

    TernaryExpression {
        cond: Box<ASTNode>,
        true_expr: Box<ASTNode>,
        false_expr: Box<ASTNode>,
    },

    CommaOperator {
        exprs: Vec<Box<ASTNode>>,
    },

    TypecastExpression {
        typespec: TypeSpec,
        expr: Box<ASTNode>,
    },

    TypeConstructor {
        typespec: TypeSpec,
        args: Vec<Box<ASTNode>>,
    },

    FunctionCall {
        name: String,
        args: Vec<Box<ASTNode>>,
        /// Per-arg read bitmask (C++ ast.h:1018 m_argread). Bit i = arg i is read.
        /// Set by typecheck_builtin_specialcase for functions with special R/W semantics.
        argread: u32,
        /// Per-arg write bitmask (C++ ast.h:1019 m_argwrite). Bit i = arg i is written.
        argwrite: u32,
        /// Per-arg derivative bitmask (C++ m_argtakesderivs). Bit i = arg i takes derivs.
        /// Set during typecheck for functions that accept derivatives.
        argtakesderivs: u32,
    },

    Literal {
        value: LiteralValue,
    },

    // ----- Statements -----
    ConditionalStatement {
        cond: Box<ASTNode>,
        true_stmt: Box<ASTNode>,
        false_stmt: Option<Box<ASTNode>>,
    },

    LoopStatement {
        loop_type: LoopType,
        init: Option<Box<ASTNode>>,
        cond: Option<Box<ASTNode>>,
        iter: Option<Box<ASTNode>>,
        body: Box<ASTNode>,
    },

    LoopModStatement {
        mod_type: LoopMod,
    },

    ReturnStatement {
        expr: Option<Box<ASTNode>>,
    },

    /// A block of statements: { stmt1; stmt2; ... }
    CompoundStatement {
        statements: Vec<Box<ASTNode>>,
    },

    /// Flat list of statements (no new scope) - e.g. multi-var declarations
    StatementList {
        statements: Vec<Box<ASTNode>>,
    },

    /// An empty statement (just a semicolon).
    EmptyStatement,

    /// Struct declaration: `struct name { type field; ... };`
    StructDeclaration {
        name: String,
        fields: Vec<Box<ASTNode>>, // Each is a VariableDeclaration
    },
}

// ---------------------------------------------------------------------------
// Node ID allocator
// ---------------------------------------------------------------------------

/// Simple monotonic node ID allocator.
pub struct NodeIdAllocator {
    next: NodeId,
}

impl NodeIdAllocator {
    pub fn new() -> Self {
        Self { next: 1 }
    }

    pub fn alloc(&mut self) -> NodeId {
        let id = self.next;
        self.next += 1;
        id
    }
}

impl Default for NodeIdAllocator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helper constructors
// ---------------------------------------------------------------------------

impl ASTNode {
    /// Create a literal int node.
    pub fn int_literal(alloc: &mut NodeIdAllocator, val: i32, loc: SourceLoc) -> Box<Self> {
        Box::new(ASTNode::new(
            alloc.alloc(),
            ASTNodeKind::Literal {
                value: LiteralValue::Int(val),
            },
            loc,
        ))
    }

    /// Create a literal float node.
    pub fn float_literal(alloc: &mut NodeIdAllocator, val: f32, loc: SourceLoc) -> Box<Self> {
        Box::new(ASTNode::new(
            alloc.alloc(),
            ASTNodeKind::Literal {
                value: LiteralValue::Float(val),
            },
            loc,
        ))
    }

    /// Create a literal string node.
    pub fn string_literal(alloc: &mut NodeIdAllocator, val: String, loc: SourceLoc) -> Box<Self> {
        Box::new(ASTNode::new(
            alloc.alloc(),
            ASTNodeKind::Literal {
                value: LiteralValue::String(val),
            },
            loc,
        ))
    }

    /// Create a variable reference node.
    pub fn var_ref(alloc: &mut NodeIdAllocator, name: String, loc: SourceLoc) -> Box<Self> {
        Box::new(ASTNode::new(
            alloc.alloc(),
            ASTNodeKind::VariableRef { name },
            loc,
        ))
    }

    /// Create a binary expression node.
    pub fn binary(
        alloc: &mut NodeIdAllocator,
        op: Operator,
        left: Box<Self>,
        right: Box<Self>,
        loc: SourceLoc,
    ) -> Box<Self> {
        Box::new(ASTNode::new(
            alloc.alloc(),
            ASTNodeKind::BinaryExpression { op, left, right },
            loc,
        ))
    }

    /// Create a function call node.
    pub fn call(
        alloc: &mut NodeIdAllocator,
        name: String,
        args: Vec<Box<Self>>,
        loc: SourceLoc,
    ) -> Box<Self> {
        Box::new(ASTNode::new(
            alloc.alloc(),
            ASTNodeKind::FunctionCall {
                name,
                args,
                argread: u32::MAX,
                argwrite: 1,
                argtakesderivs: 0,
            },
            loc,
        ))
    }
}

// ---------------------------------------------------------------------------
// AST pretty-print (matches C++ ASTNode::print)
// ---------------------------------------------------------------------------

impl ASTNode {
    /// Pretty-print this node to a string with indentation.
    pub fn print(&self, indent: usize) -> String {
        let mut out = String::new();
        self.print_impl(&mut out, indent);
        out
    }

    fn print_impl(&self, out: &mut String, indent: usize) {
        let pad = "  ".repeat(indent);
        match &self.kind {
            ASTNodeKind::ShaderDeclaration {
                shader_type,
                name,
                formals,
                statements,
                ..
            } => {
                out.push_str(&format!("{pad}{shader_type} {name}(\n"));
                for f in formals {
                    f.print_impl(out, indent + 1);
                }
                out.push_str(&format!("{pad})\n{pad}{{\n"));
                for s in statements {
                    s.print_impl(out, indent + 1);
                }
                out.push_str(&format!("{pad}}}\n"));
            }
            ASTNodeKind::FunctionDeclaration {
                name,
                return_type,
                formals,
                statements,
                ..
            } => {
                out.push_str(&format!("{pad}{:?} {name}(\n", return_type.simpletype()));
                for f in formals {
                    f.print_impl(out, indent + 1);
                }
                out.push_str(&format!("{pad})\n{pad}{{\n"));
                for s in statements {
                    s.print_impl(out, indent + 1);
                }
                out.push_str(&format!("{pad}}}\n"));
            }
            ASTNodeKind::VariableDeclaration {
                name,
                typespec,
                init,
                is_param,
                ..
            } => {
                let prefix = if *is_param { "param " } else { "" };
                out.push_str(&format!("{pad}{prefix}{:?} {name}", typespec.simpletype()));
                if let Some(init_expr) = init {
                    out.push_str(" = ");
                    init_expr.print_impl(out, 0);
                }
                out.push('\n');
            }
            ASTNodeKind::Literal { value } => match value {
                LiteralValue::Int(v) => out.push_str(&format!("{v}")),
                LiteralValue::Float(v) => out.push_str(&format!("{v}")),
                LiteralValue::String(v) => out.push_str(&format!("\"{v}\"")),
            },
            ASTNodeKind::VariableRef { name } => out.push_str(name),
            ASTNodeKind::BinaryExpression { op, left, right } => {
                out.push('(');
                left.print_impl(out, 0);
                out.push_str(&format!(" {op:?} "));
                right.print_impl(out, 0);
                out.push(')');
            }
            ASTNodeKind::UnaryExpression { op, expr } => {
                out.push_str(&format!("({op:?} "));
                expr.print_impl(out, 0);
                out.push(')');
            }
            ASTNodeKind::AssignExpression { op, lvalue, expr } => {
                lvalue.print_impl(out, 0);
                out.push_str(&format!(" {op:?} "));
                expr.print_impl(out, 0);
            }
            ASTNodeKind::FunctionCall { name, args, .. } => {
                out.push_str(&format!("{name}("));
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    arg.print_impl(out, 0);
                }
                out.push(')');
            }
            ASTNodeKind::TypeConstructor { typespec, args } => {
                out.push_str(&format!("{:?}(", typespec.simpletype()));
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    arg.print_impl(out, 0);
                }
                out.push(')');
            }
            ASTNodeKind::TernaryExpression {
                cond,
                true_expr,
                false_expr,
            } => {
                out.push('(');
                cond.print_impl(out, 0);
                out.push_str(" ? ");
                true_expr.print_impl(out, 0);
                out.push_str(" : ");
                false_expr.print_impl(out, 0);
                out.push(')');
            }
            ASTNodeKind::ConditionalStatement {
                cond,
                true_stmt,
                false_stmt,
            } => {
                out.push_str(&format!("{pad}if ("));
                cond.print_impl(out, 0);
                out.push_str(&format!(")\n{pad}{{\n"));
                true_stmt.print_impl(out, indent + 1);
                out.push_str(&format!("{pad}}}"));
                if let Some(fs) = false_stmt {
                    out.push_str(&format!(" else {{\n"));
                    fs.print_impl(out, indent + 1);
                    out.push_str(&format!("{pad}}}"));
                }
                out.push('\n');
            }
            ASTNodeKind::LoopStatement {
                loop_type,
                init,
                cond,
                iter,
                body,
            } => {
                match loop_type {
                    LoopType::For => {
                        out.push_str(&format!("{pad}for ("));
                        if let Some(i) = init {
                            i.print_impl(out, 0);
                        }
                        out.push_str("; ");
                        if let Some(c) = cond {
                            c.print_impl(out, 0);
                        }
                        out.push_str("; ");
                        if let Some(it) = iter {
                            it.print_impl(out, 0);
                        }
                        out.push_str(&format!(")\n{pad}{{\n"));
                    }
                    LoopType::While => {
                        out.push_str(&format!("{pad}while ("));
                        if let Some(c) = cond {
                            c.print_impl(out, 0);
                        }
                        out.push_str(&format!(")\n{pad}{{\n"));
                    }
                    LoopType::DoWhile => {
                        out.push_str(&format!("{pad}do {{\n"));
                    }
                }
                body.print_impl(out, indent + 1);
                if *loop_type == LoopType::DoWhile {
                    out.push_str(&format!("{pad}}} while ("));
                    if let Some(c) = cond {
                        c.print_impl(out, 0);
                    }
                    out.push_str(");\n");
                } else {
                    out.push_str(&format!("{pad}}}\n"));
                }
            }
            ASTNodeKind::CompoundStatement { statements }
            | ASTNodeKind::StatementList { statements } => {
                for s in statements {
                    s.print_impl(out, indent);
                }
            }
            ASTNodeKind::ReturnStatement { expr } => {
                out.push_str(&format!("{pad}return"));
                if let Some(e) = expr {
                    out.push(' ');
                    e.print_impl(out, 0);
                }
                out.push('\n');
            }
            ASTNodeKind::LoopModStatement { mod_type } => {
                let kw = match mod_type {
                    LoopMod::Break => "break",
                    LoopMod::Continue => "continue",
                };
                out.push_str(&format!("{pad}{kw}\n"));
            }
            ASTNodeKind::Index { base, index, .. } => {
                base.print_impl(out, 0);
                out.push('[');
                index.print_impl(out, 0);
                out.push(']');
            }
            ASTNodeKind::StructSelect { base, field } => {
                base.print_impl(out, 0);
                out.push('.');
                out.push_str(field);
            }
            ASTNodeKind::PreIncDec { op, expr } => {
                let sym = if *op == Operator::PreInc { "++" } else { "--" };
                out.push_str(sym);
                expr.print_impl(out, 0);
            }
            ASTNodeKind::PostIncDec { op, expr } => {
                expr.print_impl(out, 0);
                let sym = if *op == Operator::PostInc { "++" } else { "--" };
                out.push_str(sym);
            }
            ASTNodeKind::CompoundInitializer { elements, .. } => {
                out.push_str("{ ");
                for (i, e) in elements.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    e.print_impl(out, 0);
                }
                out.push_str(" }");
            }
            ASTNodeKind::CommaOperator { exprs } => {
                out.push('(');
                for (i, e) in exprs.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    e.print_impl(out, 0);
                }
                out.push(')');
            }
            ASTNodeKind::TypecastExpression { typespec, expr } => {
                out.push_str(&format!("({:?})", typespec.simpletype()));
                expr.print_impl(out, 0);
            }
            ASTNodeKind::StructDeclaration { name, fields } => {
                out.push_str(&format!("{pad}struct {name} {{\n"));
                for f in fields {
                    f.print_impl(out, indent + 1);
                }
                out.push_str(&format!("{pad}}};\n"));
            }
            ASTNodeKind::EmptyStatement => {
                out.push_str(&format!("{pad};\n"));
            }
        }
    }

    /// Walk the AST and call `visitor` for every node, depth-first.
    pub fn walk<F: FnMut(&ASTNode)>(&self, visitor: &mut F) {
        visitor(self);
        match &self.kind {
            ASTNodeKind::ShaderDeclaration {
                formals,
                statements,
                ..
            } => {
                for f in formals {
                    f.walk(visitor);
                }
                for s in statements {
                    s.walk(visitor);
                }
            }
            ASTNodeKind::FunctionDeclaration {
                formals,
                statements,
                ..
            } => {
                for f in formals {
                    f.walk(visitor);
                }
                for s in statements {
                    s.walk(visitor);
                }
            }
            ASTNodeKind::VariableDeclaration { init, metadata, .. } => {
                if let Some(i) = init {
                    i.walk(visitor);
                }
                for m in metadata {
                    m.walk(visitor);
                }
            }
            ASTNodeKind::BinaryExpression { left, right, .. } => {
                left.walk(visitor);
                right.walk(visitor);
            }
            ASTNodeKind::UnaryExpression { expr, .. } => expr.walk(visitor),
            ASTNodeKind::AssignExpression { lvalue, expr, .. } => {
                lvalue.walk(visitor);
                expr.walk(visitor);
            }
            ASTNodeKind::FunctionCall { args, .. } => {
                for a in args {
                    a.walk(visitor);
                }
            }
            ASTNodeKind::TypeConstructor { args, .. } => {
                for a in args {
                    a.walk(visitor);
                }
            }
            ASTNodeKind::TernaryExpression {
                cond,
                true_expr,
                false_expr,
            } => {
                cond.walk(visitor);
                true_expr.walk(visitor);
                false_expr.walk(visitor);
            }
            ASTNodeKind::ConditionalStatement {
                cond,
                true_stmt,
                false_stmt,
            } => {
                cond.walk(visitor);
                true_stmt.walk(visitor);
                if let Some(f) = false_stmt {
                    f.walk(visitor);
                }
            }
            ASTNodeKind::LoopStatement {
                init,
                cond,
                iter,
                body,
                ..
            } => {
                if let Some(i) = init {
                    i.walk(visitor);
                }
                if let Some(c) = cond {
                    c.walk(visitor);
                }
                if let Some(it) = iter {
                    it.walk(visitor);
                }
                body.walk(visitor);
            }
            ASTNodeKind::CompoundStatement { statements }
            | ASTNodeKind::StatementList { statements } => {
                for s in statements {
                    s.walk(visitor);
                }
            }
            ASTNodeKind::ReturnStatement { expr } => {
                if let Some(e) = expr {
                    e.walk(visitor);
                }
            }
            ASTNodeKind::Index {
                base,
                index,
                index2,
                index3,
            } => {
                base.walk(visitor);
                index.walk(visitor);
                if let Some(i2) = index2 {
                    i2.walk(visitor);
                }
                if let Some(i3) = index3 {
                    i3.walk(visitor);
                }
            }
            ASTNodeKind::StructSelect { base, .. } => base.walk(visitor),
            ASTNodeKind::PreIncDec { expr, .. } | ASTNodeKind::PostIncDec { expr, .. } => {
                expr.walk(visitor);
            }
            ASTNodeKind::CompoundInitializer { elements, .. } => {
                for e in elements {
                    e.walk(visitor);
                }
            }
            ASTNodeKind::CommaOperator { exprs } => {
                for e in exprs {
                    e.walk(visitor);
                }
            }
            ASTNodeKind::TypecastExpression { expr, .. } => expr.walk(visitor),
            ASTNodeKind::StructDeclaration { fields, .. } => {
                for f in fields {
                    f.walk(visitor);
                }
            }
            _ => {} // Literal, VariableRef, LoopMod, EmptyStatement -- leaf nodes
        }
    }

    /// Count total nodes in this subtree.
    pub fn node_count(&self) -> usize {
        let mut count = 0;
        self.walk(&mut |_| count += 1);
        count
    }

    /// Check if this node is an lvalue (can appear on the left side of =).
    pub fn is_lvalue(&self) -> bool {
        matches!(
            self.kind,
            ASTNodeKind::VariableRef { .. }
                | ASTNodeKind::Index { .. }
                | ASTNodeKind::StructSelect { .. }
        )
    }
}

/// Pretty-print an entire AST.
pub fn print_ast(nodes: &[Box<ASTNode>]) -> String {
    let mut out = String::new();
    for node in nodes {
        out.push_str(&node.print(0));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_allocator() {
        let mut alloc = NodeIdAllocator::new();
        assert_eq!(alloc.alloc(), 1);
        assert_eq!(alloc.alloc(), 2);
        assert_eq!(alloc.alloc(), 3);
    }

    #[test]
    fn test_literal_constructors() {
        let mut alloc = NodeIdAllocator::new();
        let loc = SourceLoc { line: 1, col: 1 };

        let int_node = ASTNode::int_literal(&mut alloc, 42, loc);
        assert!(matches!(
            int_node.kind,
            ASTNodeKind::Literal {
                value: LiteralValue::Int(42)
            }
        ));

        let float_node = ASTNode::float_literal(&mut alloc, 3.14, loc);
        assert!(
            matches!(float_node.kind, ASTNodeKind::Literal { value: LiteralValue::Float(v) } if (v - 3.14).abs() < 0.01)
        );
    }

    #[test]
    fn test_binary_expr() {
        let mut alloc = NodeIdAllocator::new();
        let loc = SourceLoc { line: 1, col: 1 };

        let left = ASTNode::int_literal(&mut alloc, 1, loc);
        let right = ASTNode::int_literal(&mut alloc, 2, loc);
        let expr = ASTNode::binary(&mut alloc, Operator::Add, left, right, loc);

        assert!(matches!(
            expr.kind,
            ASTNodeKind::BinaryExpression {
                op: Operator::Add,
                ..
            }
        ));
    }

    #[test]
    fn test_operator_variants() {
        assert_ne!(Operator::Add, Operator::Sub);
        assert_eq!(Operator::Assign, Operator::Assign);
    }
}
