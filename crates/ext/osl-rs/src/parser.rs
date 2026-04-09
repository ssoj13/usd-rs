//! Recursive-descent parser for OSL source code.
//!
//! Parses tokens from the logos-based lexer into an AST. Implements the full
//! OSL grammar as specified in `oslgram.y`.

use crate::ast::*;
use crate::lexer::{OslLexer, SourceLoc, Tok, offset_to_loc};
use crate::typedesc::{Aggregate, BaseType, TypeDesc, VecSemantics};
use crate::typespec::{StructSpec, TypeSpec, find_struct_by_name, register_struct};
use crate::ustring::UString;

// Helper to create array TypeDesc
fn typedesc_array(td: TypeDesc, arraylen: i32) -> TypeDesc {
    TypeDesc {
        basetype: td.basetype,
        aggregate: td.aggregate,
        vecsemantics: td.vecsemantics,
        reserved: 0,
        arraylen,
    }
}

/// Parse error.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub loc: SourceLoc,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.loc, self.message)
    }
}

impl std::error::Error for ParseError {}

type ParseResult<T> = Result<T, ParseError>;

/// The OSL parser (logos-powered).
pub struct Parser<'a> {
    source: &'a str,
    tokens: Vec<(Tok, usize)>, // (token, byte_offset)
    pos: usize,
    /// Node ID allocator.
    alloc: NodeIdAllocator,
    /// Collected errors (non-fatal).
    pub errors: Vec<ParseError>,
    /// Collected warnings (non-fatal diagnostics).
    pub warnings: Vec<String>,
    /// True while parsing shader formal parameters (allows empty `{}` init).
    in_shader_formals: bool,
}

impl<'a> Parser<'a> {
    /// Create a new parser from source text.
    pub fn new(source: &'a str) -> Self {
        let mut lexer = OslLexer::new(source);
        let mut tokens = Vec::new();
        for item in lexer.by_ref() {
            match item {
                Ok((start, tok, _end)) => tokens.push((tok, start)),
                Err(e) => tokens.push((Tok::Identifier(format!("<error@{}>", e.loc)), e.loc)),
            }
        }
        let errors: Vec<ParseError> = lexer
            .errors
            .into_iter()
            .map(|(loc, msg)| ParseError { message: msg, loc })
            .collect();
        // C++ parity: string_literal_group — merge adjacent STRING_LITERAL tokens
        let tokens = Self::merge_string_literals(tokens);
        Self {
            source,
            tokens,
            pos: 0,
            alloc: NodeIdAllocator::new(),
            errors,
            warnings: Vec::new(),
            in_shader_formals: false,
        }
    }

    /// Merge adjacent StringLiteral tokens (C++ parity: oslgram.y string_literal_group).
    fn merge_string_literals(mut tokens: Vec<(Tok, usize)>) -> Vec<(Tok, usize)> {
        let mut i = 0;
        while i < tokens.len() {
            if let Tok::StringLiteral(s1) = &tokens[i].0 {
                let mut merged = s1.clone();
                let start = tokens[i].1;
                while i + 1 < tokens.len() {
                    if let Tok::StringLiteral(s2) = &tokens[i + 1].0 {
                        merged.push_str(s2);
                        tokens.remove(i + 1);
                    } else {
                        break;
                    }
                }
                tokens[i] = (Tok::StringLiteral(merged), start);
            }
            i += 1;
        }
        tokens
    }

    // ----- Token helpers -----

    fn current(&self) -> &Tok {
        if self.pos < self.tokens.len() {
            &self.tokens[self.pos].0
        } else {
            // sentinel — treat as EOF
            static EOF: Tok = Tok::Semi; // never matched as EOF; at_eof checks pos
            &EOF
        }
    }

    fn current_offset(&self) -> usize {
        if self.pos < self.tokens.len() {
            self.tokens[self.pos].1
        } else {
            self.source.len()
        }
    }

    fn peek(&self) -> &Tok {
        let next = self.pos + 1;
        if next < self.tokens.len() {
            &self.tokens[next].0
        } else {
            static EOF: Tok = Tok::Semi;
            &EOF
        }
    }

    fn loc(&self) -> SourceLoc {
        offset_to_loc(self.source, self.current_offset())
    }

    fn advance(&mut self) -> Tok {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].0.clone();
            self.pos += 1;
            tok
        } else {
            Tok::Semi // should never happen
        }
    }

    fn at(&self, kind: &Tok) -> bool {
        if self.at_eof() {
            return false;
        }
        std::mem::discriminant(self.current()) == std::mem::discriminant(kind)
    }

    fn at_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn expect(&mut self, kind: &Tok) -> ParseResult<Tok> {
        if self.at(kind) {
            Ok(self.advance())
        } else {
            Err(self.error(format!(
                "expected {:?}, found {:?}",
                kind,
                if self.at_eof() { "EOF" } else { "" }
            )))
        }
    }

    fn eat(&mut self, kind: &Tok) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn error(&self, msg: String) -> ParseError {
        ParseError {
            message: msg,
            loc: self.loc(),
        }
    }

    // ----- Type parsing -----

    fn is_type_keyword(&self) -> bool {
        Self::token_is_type(self.current())
    }

    fn peek_is_type_keyword(&self) -> bool {
        Self::token_is_type(self.peek())
    }

    fn at_metadata_bracket(&self) -> bool {
        self.at(&Tok::MetadataBegin)
    }

    /// Expect `] ]` (two right brackets) to close a metadata block.
    /// C++ uses two separate `]` tokens, not a single `]]` token.
    fn expect_metadata_end(&mut self) -> ParseResult<()> {
        self.expect(&Tok::RBracket)?;
        self.expect(&Tok::RBracket)?;
        Ok(())
    }

    fn peek_could_be_decl_after_type(&self) -> bool {
        matches!(self.peek(), Tok::Identifier(_))
    }

    fn token_is_type(tok: &Tok) -> bool {
        matches!(
            tok,
            Tok::IntType
                | Tok::FloatType
                | Tok::StringType
                | Tok::ColorType
                | Tok::PointType
                | Tok::VectorType
                | Tok::NormalType
                | Tok::MatrixType
                | Tok::VoidType
        )
    }

    fn parse_simple_type(&mut self) -> ParseResult<TypeSpec> {
        // Check for struct type name first: look it up in the global struct registry.
        if let Tok::Identifier(ident) = self.current().clone() {
            let sid = find_struct_by_name(UString::new(&ident));
            if sid > 0 {
                self.advance();
                let arraylen = if self.eat(&Tok::LBracket) {
                    if let Tok::IntLiteral(n) | Tok::OctalLiteral(n) = self.current() {
                        let n = *n;
                        self.advance();
                        self.expect(&Tok::RBracket)?;
                        n
                    } else if self.at(&Tok::RBracket) {
                        self.advance();
                        -1
                    } else {
                        return Err(self.error("expected array size or ']'".into()));
                    }
                } else {
                    0
                };
                return Ok(TypeSpec::structure(sid as i16, arraylen));
            }
        }

        let td = match self.current() {
            Tok::IntType => TypeDesc::INT,
            Tok::FloatType => TypeDesc::FLOAT,
            Tok::StringType => TypeDesc::STRING,
            Tok::ColorType => TypeDesc::new(BaseType::Float, Aggregate::Vec3, VecSemantics::Color),
            Tok::PointType => TypeDesc::new(BaseType::Float, Aggregate::Vec3, VecSemantics::Point),
            Tok::VectorType => {
                TypeDesc::new(BaseType::Float, Aggregate::Vec3, VecSemantics::Vector)
            }
            Tok::NormalType => {
                TypeDesc::new(BaseType::Float, Aggregate::Vec3, VecSemantics::Normal)
            }
            Tok::MatrixType => TypeDesc::MATRIX,
            Tok::VoidType => TypeDesc::NONE,
            Tok::Identifier(_) => TypeDesc::INT, // struct placeholder (not yet registered)
            _ => return Err(self.error(format!("expected type, found {:?}", self.current()))),
        };
        self.advance();

        let td = if self.eat(&Tok::LBracket) {
            if let Tok::IntLiteral(n) | Tok::OctalLiteral(n) = self.current() {
                let n = *n;
                self.advance();
                self.expect(&Tok::RBracket)?;
                typedesc_array(td, n)
            } else if self.at(&Tok::RBracket) {
                self.advance();
                typedesc_array(td, -1)
            } else {
                return Err(self.error("expected array size or ']'".into()));
            }
        } else {
            td
        };

        Ok(TypeSpec::from_simple(td))
    }

    fn parse_typespec(&mut self) -> ParseResult<TypeSpec> {
        if self.eat(&Tok::Closure) {
            let inner = self.parse_simple_type()?;
            return Ok(TypeSpec::closure(inner.simpletype()));
        }
        self.parse_simple_type()
    }

    // ----- Top-level parsing -----

    pub fn parse_shader_file(&mut self) -> ParseResult<Vec<ASTNode>> {
        let mut declarations = Vec::new();

        while !self.at_eof() {
            match self.parse_declaration() {
                Ok(node) => declarations.push(*node),
                Err(e) => {
                    self.errors.push(e);
                    self.recover();
                }
            }
        }

        Ok(declarations)
    }

    fn parse_declaration(&mut self) -> ParseResult<Box<ASTNode>> {
        match self.current() {
            Tok::Shader | Tok::Surface | Tok::Displacement | Tok::Volume => {
                self.parse_shader_declaration()
            }
            Tok::Struct => self.parse_struct_declaration(),
            _ if self.is_type_keyword() || self.at(&Tok::Closure) => {
                self.parse_function_or_variable_declaration()
            }
            Tok::Identifier(_) if self.peek_could_be_decl_after_type() => {
                self.parse_function_or_variable_declaration()
            }
            _ => Err(self.error(format!(
                "unexpected token {:?} at top level",
                self.current()
            ))),
        }
    }

    fn parse_struct_declaration(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        self.expect(&Tok::Struct)?;
        let name = self.expect_identifier()?;
        self.expect(&Tok::LBrace)?;

        let mut fields = Vec::new();
        while !self.at(&Tok::RBrace) && !self.at_eof() {
            let mut field_group = self.parse_struct_fields()?;
            fields.append(&mut field_group);
        }
        self.expect(&Tok::RBrace)?;
        self.expect(&Tok::Semi)?;

        // Register this struct in the global type registry so that later uses of
        // the struct name as a type (e.g. `MyStruct var;` or `MyStruct(...)`)
        // can resolve to the correct TypeSpec with a valid structure_id.
        let struct_uname = UString::new(&name);
        if find_struct_by_name(struct_uname) == 0 {
            let mut spec = StructSpec::new(struct_uname, 0);
            for field in &fields {
                if let ASTNodeKind::VariableDeclaration {
                    name: fname,
                    typespec: fts,
                    ..
                } = &field.kind
                {
                    spec.add_field(*fts, UString::new(fname));
                }
            }
            register_struct(spec);
        }

        Ok(Box::new(ASTNode::new(
            self.alloc.alloc(),
            ASTNodeKind::StructDeclaration { name, fields },
            loc,
        )))
    }

    fn parse_struct_fields(&mut self) -> ParseResult<Vec<ASTNode>> {
        let loc = self.loc();
        let ts = self.parse_typespec()?;
        let mut fields = Vec::new();

        let name = self.expect_identifier()?;
        fields.push(*self.make_struct_field(name, ts, loc)?);

        while self.eat(&Tok::Comma) {
            let nloc = self.loc();
            let nname = self.expect_identifier()?;
            fields.push(*self.make_struct_field(nname, ts, nloc)?);
        }

        self.expect(&Tok::Semi)?;
        Ok(fields)
    }

    fn make_struct_field(
        &mut self,
        name: String,
        ts: TypeSpec,
        loc: SourceLoc,
    ) -> ParseResult<Box<ASTNode>> {
        let final_ts = if self.at(&Tok::LBracket) && !self.at_metadata_bracket() {
            self.advance();
            let arraylen = if let Tok::IntLiteral(v) | Tok::OctalLiteral(v) = self.current() {
                let v = *v;
                self.advance();
                v
            } else {
                0
            };
            self.expect(&Tok::RBracket)?;
            let mut td = ts.simpletype();
            td.arraylen = arraylen;
            TypeSpec::from_simple(td)
        } else {
            ts
        };

        let init = if self.at(&Tok::Eq) {
            self.advance();
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(Box::new(ASTNode::new(
            self.alloc.alloc(),
            ASTNodeKind::VariableDeclaration {
                name,
                typespec: final_ts,
                init,
                is_param: false,
                is_output: false,
                is_metadata: false,
                metadata: Vec::new(),
            },
            loc,
        )))
    }

    fn parse_shader_declaration(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let shader_type = match self.current() {
            Tok::Shader => "shader",
            Tok::Surface => "surface",
            Tok::Displacement => "displacement",
            Tok::Volume => "volume",
            _ => unreachable!(),
        }
        .to_string();
        self.advance();

        let name = self.expect_identifier()?;

        let mut metadata = Vec::new();
        if self.at_metadata_bracket() {
            self.advance();
            metadata = self.parse_metadata_list()?;
            self.expect_metadata_end()?;
        }

        // Mark shader formals context (allows empty `{}` initializers per C++ parity)
        self.in_shader_formals = true;
        let formals = self.parse_formal_params()?;
        self.in_shader_formals = false;

        if self.at_metadata_bracket() {
            self.advance();
            let mut m2 = self.parse_metadata_list()?;
            self.expect_metadata_end()?;
            metadata.append(&mut m2);
        }

        let statements = self.parse_compound_statement()?;

        Ok(Box::new(ASTNode::new(
            self.alloc.alloc(),
            ASTNodeKind::ShaderDeclaration {
                shader_type,
                name,
                formals,
                statements: vec![*statements],
                metadata,
            },
            loc,
        )))
    }

    fn parse_function_or_variable_declaration(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let typespec = self.parse_typespec()?;
        let name = self.expect_identifier()?;

        if self.at(&Tok::LParen) {
            return self.parse_function_declaration(name, typespec, loc);
        }

        let decl = self.parse_variable_declaration_rest(name, typespec, false, false, loc)?;
        self.expect(&Tok::Semi)?;
        Ok(decl)
    }

    fn parse_function_declaration(
        &mut self,
        name: String,
        return_type: TypeSpec,
        loc: SourceLoc,
    ) -> ParseResult<Box<ASTNode>> {
        let formals = self.parse_formal_params()?;

        // Metadata can appear before `;` or `{`
        let metadata = if self.at_metadata_bracket() {
            self.advance();
            let m = self.parse_metadata_list()?;
            self.expect_metadata_end()?;
            m
        } else {
            Vec::new()
        };

        // Forward declaration: `closure color foo(...) [[metadata]];`
        if self.eat(&Tok::Semi) {
            return Ok(Box::new(ASTNode::new(
                self.alloc.alloc(),
                ASTNodeKind::FunctionDeclaration {
                    name,
                    return_type,
                    formals,
                    statements: Vec::new(),
                    is_builtin: false,
                    metadata,
                },
                loc,
            )));
        }

        let statements = self.parse_compound_statement()?;

        Ok(Box::new(ASTNode::new(
            self.alloc.alloc(),
            ASTNodeKind::FunctionDeclaration {
                name,
                return_type,
                formals,
                statements: vec![*statements],
                metadata,
                is_builtin: false,
            },
            loc,
        )))
    }

    // ----- Parameter/formal parsing -----

    fn parse_formal_params(&mut self) -> ParseResult<Vec<ASTNode>> {
        self.expect(&Tok::LParen)?;
        let mut params = Vec::new();

        if !self.at(&Tok::RParen) {
            params.push(*self.parse_formal_param()?);
            while self.eat(&Tok::Comma) {
                if self.at(&Tok::RParen) {
                    break;
                }
                params.push(*self.parse_formal_param()?);
            }
        }

        self.expect(&Tok::RParen)?;
        Ok(params)
    }

    fn parse_formal_param(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let is_output = self.eat(&Tok::Output);
        let typespec = self.parse_typespec()?;
        let name = self.expect_identifier()?;
        self.parse_variable_declaration_rest(name, typespec, true, is_output, loc)
    }

    fn parse_variable_declaration_rest(
        &mut self,
        name: String,
        typespec: TypeSpec,
        is_param: bool,
        is_output: bool,
        loc: SourceLoc,
    ) -> ParseResult<Box<ASTNode>> {
        let typespec = if self.at(&Tok::LBracket) && !self.at_metadata_bracket() {
            self.advance();
            if let Tok::IntLiteral(n) | Tok::OctalLiteral(n) = self.current() {
                let n = *n;
                self.advance();
                self.expect(&Tok::RBracket)?;
                // Preserve closure flag when forming an array type
                if typespec.is_closure_based() {
                    TypeSpec::closure_array(n)
                } else {
                    TypeSpec::from_simple(typedesc_array(typespec.simpletype(), n))
                }
            } else if self.at(&Tok::RBracket) {
                self.advance();
                if typespec.is_closure_based() {
                    TypeSpec::closure_array(-1)
                } else {
                    TypeSpec::from_simple(typedesc_array(typespec.simpletype(), -1))
                }
            } else {
                return Err(self.error("expected array size or ']'".into()));
            }
        } else {
            typespec
        };

        let init = if self.eat(&Tok::Eq) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        let metadata = if self.at_metadata_bracket() {
            self.advance(); // consume [[ (MetadataBegin)
            let m = self.parse_metadata_list()?;
            self.expect_metadata_end()?; // consume ]]
            m
        } else {
            Vec::new()
        };

        Ok(Box::new(ASTNode::new(
            self.alloc.alloc(),
            ASTNodeKind::VariableDeclaration {
                name,
                typespec,
                init,
                is_param,
                is_output,
                is_metadata: false,
                metadata,
            },
            loc,
        )))
    }

    fn parse_metadata_list(&mut self) -> ParseResult<Vec<ASTNode>> {
        let mut metadata = Vec::new();

        loop {
            // Stop at `]` (metadata end uses two `]` tokens) or EOF
            if self.at(&Tok::RBracket) || self.at_eof() {
                break;
            }

            if self.is_type_keyword() || self.at(&Tok::StringType) {
                let loc = self.loc();
                let ts = self.parse_typespec()?;
                let name = self.expect_identifier()?;

                // Handle array metadata: `string s[2] = { "foo", "bar" }`
                let ts = if self.at(&Tok::LBracket) {
                    self.advance();
                    if let Tok::IntLiteral(n) | Tok::OctalLiteral(n) = self.current() {
                        let n = *n;
                        self.advance();
                        self.expect(&Tok::RBracket)?;
                        TypeSpec::from_simple(typedesc_array(ts.simpletype(), n))
                    } else if self.at(&Tok::RBracket) {
                        self.advance();
                        TypeSpec::from_simple(typedesc_array(ts.simpletype(), -1))
                    } else {
                        return Err(self.error("expected array size or ']'".into()));
                    }
                } else {
                    ts
                };

                let init = if self.eat(&Tok::Eq) {
                    Some(self.parse_expression()?)
                } else {
                    None
                };

                metadata.push(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::VariableDeclaration {
                        name,
                        typespec: ts,
                        init,
                        is_param: false,
                        is_output: false,
                        is_metadata: true,
                        metadata: Vec::new(),
                    },
                    loc,
                ));

                if !self.eat(&Tok::Comma) {
                    break;
                }
            } else {
                self.advance();
            }
        }

        Ok(metadata)
    }

    // ----- Statement parsing -----

    fn parse_compound_statement(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        self.expect(&Tok::LBrace)?;

        let mut statements = Vec::new();
        while !self.at(&Tok::RBrace) && !self.at_eof() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(*stmt),
                Err(e) => {
                    self.errors.push(e);
                    self.recover_to_next_statement();
                }
            }
        }

        self.expect(&Tok::RBrace)?;

        Ok(Box::new(ASTNode::new(
            self.alloc.alloc(),
            ASTNodeKind::CompoundStatement { statements },
            loc,
        )))
    }

    fn parse_statement(&mut self) -> ParseResult<Box<ASTNode>> {
        match self.current().clone() {
            Tok::LBrace => self.parse_compound_statement(),
            Tok::If => self.parse_if_statement(),
            Tok::For => self.parse_for_statement(),
            Tok::While => self.parse_while_statement(),
            Tok::Do => self.parse_do_while_statement(),
            Tok::Return => self.parse_return_statement(),
            Tok::Break => {
                let loc = self.loc();
                self.advance();
                self.expect(&Tok::Semi)?;
                Ok(Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::LoopModStatement {
                        mod_type: LoopMod::Break,
                    },
                    loc,
                )))
            }
            Tok::Continue => {
                let loc = self.loc();
                self.advance();
                self.expect(&Tok::Semi)?;
                Ok(Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::LoopModStatement {
                        mod_type: LoopMod::Continue,
                    },
                    loc,
                )))
            }
            Tok::Semi => {
                let loc = self.loc();
                self.advance();
                Ok(Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::EmptyStatement,
                    loc,
                )))
            }
            _ if self.is_type_keyword() || self.at(&Tok::Closure) || self.at(&Tok::Output) => {
                self.parse_local_declaration()
            }
            Tok::Identifier(_) if self.peek_could_be_decl_after_type() => {
                self.parse_local_declaration()
            }
            _ => {
                let expr = self.parse_expression()?;
                self.expect(&Tok::Semi)?;
                Ok(expr)
            }
        }
    }

    fn parse_if_statement(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        self.expect(&Tok::If)?;
        self.expect(&Tok::LParen)?;
        let cond = self.parse_expression()?;
        self.expect(&Tok::RParen)?;
        let true_stmt = self.parse_statement()?;
        let false_stmt = if self.eat(&Tok::Else) {
            Some(self.parse_statement()?)
        } else {
            None
        };

        Ok(Box::new(ASTNode::new(
            self.alloc.alloc(),
            ASTNodeKind::ConditionalStatement {
                cond,
                true_stmt,
                false_stmt,
            },
            loc,
        )))
    }

    fn parse_for_statement(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        self.expect(&Tok::For)?;
        self.expect(&Tok::LParen)?;

        let init = if self.at(&Tok::Semi) {
            self.advance();
            None
        } else if self.is_type_keyword() {
            Some(self.parse_local_declaration()?)
        } else {
            let e = self.parse_expression()?;
            self.expect(&Tok::Semi)?;
            Some(e)
        };

        let cond = if self.at(&Tok::Semi) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(&Tok::Semi)?;

        let iter = if self.at(&Tok::RParen) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(&Tok::RParen)?;

        let body = self.parse_statement()?;

        Ok(Box::new(ASTNode::new(
            self.alloc.alloc(),
            ASTNodeKind::LoopStatement {
                loop_type: LoopType::For,
                init,
                cond,
                iter,
                body,
            },
            loc,
        )))
    }

    fn parse_while_statement(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        self.expect(&Tok::While)?;
        self.expect(&Tok::LParen)?;
        let cond = Some(self.parse_expression()?);
        self.expect(&Tok::RParen)?;
        let body = self.parse_statement()?;

        Ok(Box::new(ASTNode::new(
            self.alloc.alloc(),
            ASTNodeKind::LoopStatement {
                loop_type: LoopType::While,
                init: None,
                cond,
                iter: None,
                body,
            },
            loc,
        )))
    }

    fn parse_do_while_statement(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        self.expect(&Tok::Do)?;
        let body = self.parse_statement()?;
        self.expect(&Tok::While)?;
        self.expect(&Tok::LParen)?;
        let cond = Some(self.parse_expression()?);
        self.expect(&Tok::RParen)?;
        self.expect(&Tok::Semi)?;

        Ok(Box::new(ASTNode::new(
            self.alloc.alloc(),
            ASTNodeKind::LoopStatement {
                loop_type: LoopType::DoWhile,
                init: None,
                cond,
                iter: None,
                body,
            },
            loc,
        )))
    }

    fn parse_return_statement(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        self.expect(&Tok::Return)?;
        let expr = if self.at(&Tok::Semi) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(&Tok::Semi)?;

        Ok(Box::new(ASTNode::new(
            self.alloc.alloc(),
            ASTNodeKind::ReturnStatement { expr },
            loc,
        )))
    }

    fn parse_local_declaration(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let is_output = self.eat(&Tok::Output);
        let typespec = self.parse_typespec()?;
        let name = self.expect_identifier()?;

        if self.at(&Tok::LParen) {
            return self.parse_function_declaration(name, typespec, loc);
        }

        let first = self.parse_variable_declaration_rest(name, typespec, false, is_output, loc)?;

        if !self.at(&Tok::Comma) {
            self.expect(&Tok::Semi)?;
            return Ok(first);
        }

        let mut stmts = vec![*first];
        while self.eat(&Tok::Comma) {
            let dloc = self.loc();
            let dname = self.expect_identifier()?;
            let decl =
                self.parse_variable_declaration_rest(dname, typespec, false, is_output, dloc)?;
            stmts.push(*decl);
        }

        self.expect(&Tok::Semi)?;

        Ok(Box::new(ASTNode::new(
            self.alloc.alloc(),
            ASTNodeKind::StatementList { statements: stmts },
            loc,
        )))
    }

    // ----- Expression parsing (precedence climbing) -----

    fn parse_expression(&mut self) -> ParseResult<Box<ASTNode>> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let mut left = self.parse_ternary()?;

        let op = match self.current() {
            Tok::Eq => Some(Operator::Assign),
            Tok::PlusAssign => Some(Operator::AddAssign),
            Tok::MinusAssign => Some(Operator::SubAssign),
            Tok::StarAssign => Some(Operator::MulAssign),
            Tok::SlashAssign => Some(Operator::DivAssign),
            Tok::AmpAssign => Some(Operator::BitAndAssign),
            Tok::PipeAssign => Some(Operator::BitOrAssign),
            Tok::CaretAssign => Some(Operator::BitXorAssign),
            Tok::ShiftLeftAssign => Some(Operator::ShlAssign),
            Tok::ShiftRightAssign => Some(Operator::ShrAssign),
            _ => None,
        };

        if let Some(op) = op {
            self.advance();
            let right = self.parse_assignment()?;
            left = Box::new(ASTNode::new(
                self.alloc.alloc(),
                ASTNodeKind::AssignExpression {
                    op,
                    lvalue: left,
                    expr: right,
                },
                loc,
            ));
        }

        Ok(left)
    }

    fn parse_ternary(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let cond = self.parse_or()?;

        if self.eat(&Tok::Question) {
            let true_expr = self.parse_expression()?;
            self.expect(&Tok::Colon)?;
            let false_expr = self.parse_ternary()?;
            Ok(Box::new(ASTNode::new(
                self.alloc.alloc(),
                ASTNodeKind::TernaryExpression {
                    cond,
                    true_expr,
                    false_expr,
                },
                loc,
            )))
        } else {
            Ok(cond)
        }
    }

    fn eat_or_keyword(&mut self) -> bool {
        if self.eat(&Tok::OrOr) {
            return true;
        }
        if let Tok::Identifier(s) = self.current()
            && s == "or"
        {
            self.advance();
            return true;
        }
        false
    }

    fn eat_and_keyword(&mut self) -> bool {
        if self.eat(&Tok::AndAnd) {
            return true;
        }
        if let Tok::Identifier(s) = self.current()
            && s == "and"
        {
            self.advance();
            return true;
        }
        false
    }

    fn parse_or(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let mut left = self.parse_and()?;
        while self.eat_or_keyword() {
            let right = self.parse_and()?;
            left = Box::new(ASTNode::new(
                self.alloc.alloc(),
                ASTNodeKind::BinaryExpression {
                    op: Operator::LogOr,
                    left,
                    right,
                },
                loc,
            ));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let mut left = self.parse_bitor()?;
        while self.eat_and_keyword() {
            let right = self.parse_bitor()?;
            left = Box::new(ASTNode::new(
                self.alloc.alloc(),
                ASTNodeKind::BinaryExpression {
                    op: Operator::LogAnd,
                    left,
                    right,
                },
                loc,
            ));
        }
        Ok(left)
    }

    fn parse_bitor(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let mut left = self.parse_bitxor()?;
        while self.eat(&Tok::Pipe) {
            let right = self.parse_bitxor()?;
            left = Box::new(ASTNode::new(
                self.alloc.alloc(),
                ASTNodeKind::BinaryExpression {
                    op: Operator::BitOr,
                    left,
                    right,
                },
                loc,
            ));
        }
        Ok(left)
    }

    fn parse_bitxor(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let mut left = self.parse_bitand()?;
        while self.eat(&Tok::Caret) {
            let right = self.parse_bitand()?;
            left = Box::new(ASTNode::new(
                self.alloc.alloc(),
                ASTNodeKind::BinaryExpression {
                    op: Operator::BitXor,
                    left,
                    right,
                },
                loc,
            ));
        }
        Ok(left)
    }

    fn parse_bitand(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let mut left = self.parse_equality()?;
        while self.eat(&Tok::Amp) {
            let right = self.parse_equality()?;
            left = Box::new(ASTNode::new(
                self.alloc.alloc(),
                ASTNodeKind::BinaryExpression {
                    op: Operator::BitAnd,
                    left,
                    right,
                },
                loc,
            ));
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.current() {
                Tok::EqEq => Operator::Eq,
                Tok::NotEq => Operator::NotEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            left = Box::new(ASTNode::new(
                self.alloc.alloc(),
                ASTNodeKind::BinaryExpression { op, left, right },
                loc,
            ));
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let mut left = self.parse_shift()?;
        loop {
            let op = match self.current() {
                Tok::Less => Operator::Less,
                Tok::Greater => Operator::Greater,
                Tok::LessEq => Operator::LessEq,
                Tok::GreaterEq => Operator::GreaterEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_shift()?;
            left = Box::new(ASTNode::new(
                self.alloc.alloc(),
                ASTNodeKind::BinaryExpression { op, left, right },
                loc,
            ));
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let mut left = self.parse_additive()?;
        loop {
            let op = match self.current() {
                Tok::ShiftLeft => Operator::Shl,
                Tok::ShiftRight => Operator::Shr,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = Box::new(ASTNode::new(
                self.alloc.alloc(),
                ASTNodeKind::BinaryExpression { op, left, right },
                loc,
            ));
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.current() {
                Tok::Plus => Operator::Add,
                Tok::Minus => Operator::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Box::new(ASTNode::new(
                self.alloc.alloc(),
                ASTNodeKind::BinaryExpression { op, left, right },
                loc,
            ));
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.current() {
                Tok::Star => Operator::Mul,
                Tok::Slash => Operator::Div,
                Tok::Percent => Operator::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Box::new(ASTNode::new(
                self.alloc.alloc(),
                ASTNodeKind::BinaryExpression { op, left, right },
                loc,
            ));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();

        match self.current().clone() {
            Tok::Plus => {
                // Unary + is a no-op (C++ oslgram.y:784)
                self.advance();
                self.parse_unary()
            }
            Tok::Minus => {
                self.advance();
                let expr = self.parse_unary()?;
                // C++ oslgram.y:786-790: negate numeric literals in-place
                if let ASTNodeKind::Literal { ref value } = expr.kind
                    && matches!(value, LiteralValue::Int(_) | LiteralValue::Float(_))
                {
                    let mut node = *expr;
                    if let ASTNodeKind::Literal { ref mut value } = node.kind {
                        value.negate();
                    }
                    return Ok(Box::new(node));
                }
                Ok(Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::UnaryExpression {
                        op: Operator::Neg,
                        expr,
                    },
                    loc,
                )))
            }
            Tok::Not => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::UnaryExpression {
                        op: Operator::Not,
                        expr,
                    },
                    loc,
                )))
            }
            Tok::Identifier(ref s) if s == "not" => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::UnaryExpression {
                        op: Operator::Not,
                        expr,
                    },
                    loc,
                )))
            }
            Tok::Tilde => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::UnaryExpression {
                        op: Operator::BitNot,
                        expr,
                    },
                    loc,
                )))
            }
            Tok::PlusPlus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::PreIncDec {
                        op: Operator::PreInc,
                        expr,
                    },
                    loc,
                )))
            }
            Tok::MinusMinus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::PreIncDec {
                        op: Operator::PreDec,
                        expr,
                    },
                    loc,
                )))
            }
            // Type constructor: color(1,0,0) etc.
            _ if self.is_type_keyword() && matches!(self.peek(), Tok::LParen) => {
                let ts = self.parse_simple_type()?;
                self.expect(&Tok::LParen)?;
                let args = self.parse_arg_list()?;
                self.expect(&Tok::RParen)?;
                Ok(Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::TypeConstructor { typespec: ts, args },
                    loc,
                )))
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();
        let mut expr = self.parse_primary()?;

        loop {
            if self.at(&Tok::LBracket) && !self.at_metadata_bracket() {
                self.advance();
                let index = self.parse_expression()?;
                self.expect(&Tok::RBracket)?;
                expr = Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::Index {
                        base: expr,
                        index,
                        index2: None,
                        index3: None,
                    },
                    loc,
                ));
            } else if self.at(&Tok::Dot) {
                self.advance();
                let field = self.expect_identifier()?;
                expr = Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::StructSelect { base: expr, field },
                    loc,
                ));
            } else if self.at(&Tok::PlusPlus) {
                self.advance();
                expr = Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::PostIncDec {
                        op: Operator::PostInc,
                        expr,
                    },
                    loc,
                ));
            } else if self.at(&Tok::MinusMinus) {
                self.advance();
                expr = Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::PostIncDec {
                        op: Operator::PostDec,
                        expr,
                    },
                    loc,
                ));
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> ParseResult<Box<ASTNode>> {
        let loc = self.loc();

        match self.current().clone() {
            Tok::IntLiteral(v) | Tok::OctalLiteral(v) => {
                self.advance();
                Ok(ASTNode::int_literal(&mut self.alloc, v, loc))
            }
            Tok::HexLiteral(v) => {
                self.advance();
                Ok(ASTNode::int_literal(&mut self.alloc, v, loc))
            }
            Tok::FloatLiteral(v) => {
                self.advance();
                Ok(ASTNode::float_literal(&mut self.alloc, v, loc))
            }
            Tok::StringLiteral(s) => {
                let mut combined = s.clone();
                self.advance();
                while let Tok::StringLiteral(s2) = self.current() {
                    combined.push_str(s2);
                    self.advance();
                }
                Ok(ASTNode::string_literal(&mut self.alloc, combined, loc))
            }
            Tok::Identifier(name) => {
                self.advance();
                if self.at(&Tok::LParen) {
                    self.advance();
                    let args = self.parse_arg_list()?;
                    self.expect(&Tok::RParen)?;
                    Ok(ASTNode::call(&mut self.alloc, name, args, loc))
                } else {
                    Ok(ASTNode::var_ref(&mut self.alloc, name, loc))
                }
            }
            Tok::LParen => {
                if self.peek_is_type_keyword() {
                    self.advance(); // consume '('
                    let ts = self.parse_simple_type()?;
                    if self.at(&Tok::RParen) {
                        // C-style cast: (type)expr
                        self.advance();
                        let expr = self.parse_unary()?;
                        return Ok(Box::new(ASTNode::new(
                            self.alloc.alloc(),
                            ASTNodeKind::TypeConstructor {
                                typespec: ts,
                                args: vec![*expr],
                            },
                            loc,
                        )));
                    }
                    if self.at(&Tok::LParen) {
                        // Parenthesized type constructor: (type(args...))
                        self.advance();
                        let args = self.parse_arg_list()?;
                        self.expect(&Tok::RParen)?;
                        let first = Box::new(ASTNode::new(
                            self.alloc.alloc(),
                            ASTNodeKind::TypeConstructor { typespec: ts, args },
                            loc,
                        ));
                        if self.at(&Tok::Comma) {
                            let mut elems = vec![*first];
                            while self.eat(&Tok::Comma) {
                                elems.push(*self.parse_expression()?);
                            }
                            self.expect(&Tok::RParen)?;
                            return Ok(Box::new(ASTNode::new(
                                self.alloc.alloc(),
                                ASTNodeKind::CompoundInitializer {
                                    elements: elems,
                                    canconstruct: false,
                                },
                                loc,
                            )));
                        }
                        self.expect(&Tok::RParen)?;
                        return Ok(first);
                    }
                    return Err(self.error(format!(
                        "expected '(' or ')' after type in parenthesized expression, found {:?}",
                        self.current()
                    )));
                }
                self.advance();
                let first = self.parse_expression()?;
                // Comma operator: (a, b, c) -> CommaOperator (C++ ASTcomma_operator)
                // NOT CompoundInitializer — that's only for {a, b, c}
                if self.at(&Tok::Comma) {
                    let mut exprs = vec![*first];
                    while self.eat(&Tok::Comma) {
                        exprs.push(*self.parse_expression()?);
                    }
                    self.expect(&Tok::RParen)?;
                    // C++ parity: warn about comma operator in parens (oslgram.y:798-807)
                    self.warnings.push(format!(
                        "{}: Comma operator inside parenthesis is probably an error -- it is not a vector/color.",
                        loc
                    ));
                    Ok(Box::new(ASTNode::new(
                        self.alloc.alloc(),
                        ASTNodeKind::CommaOperator { exprs },
                        loc,
                    )))
                } else {
                    self.expect(&Tok::RParen)?;
                    Ok(first)
                }
            }
            Tok::LBrace => {
                self.advance();
                let mut elements = Vec::new();
                if !self.at(&Tok::RBrace) {
                    elements.push(*self.parse_expression()?);
                    while self.eat(&Tok::Comma) {
                        if self.at(&Tok::RBrace) {
                            break;
                        }
                        elements.push(*self.parse_expression()?);
                    }
                } else if !self.in_shader_formals {
                    // C++ parity (oslgram.y:488-497): empty `{}` only allowed for shader params
                    self.errors.push(ParseError {
                        message:
                            "Empty compound initializers '{ }' only allowed for shader parameters."
                                .to_string(),
                        loc,
                    });
                }
                self.expect(&Tok::RBrace)?;
                Ok(Box::new(ASTNode::new(
                    self.alloc.alloc(),
                    ASTNodeKind::CompoundInitializer {
                        elements,
                        canconstruct: false,
                    },
                    loc,
                )))
            }
            _ => Err(self.error(format!(
                "unexpected token {:?} in expression",
                self.current()
            ))),
        }
    }

    fn parse_arg_list(&mut self) -> ParseResult<Vec<ASTNode>> {
        let mut args = Vec::new();
        if !self.at(&Tok::RParen) {
            args.push(*self.parse_expression()?);
            while self.eat(&Tok::Comma) {
                args.push(*self.parse_expression()?);
            }
        }
        Ok(args)
    }

    // ----- Utility -----

    fn expect_identifier(&mut self) -> ParseResult<String> {
        if let Tok::Identifier(name) = self.current().clone() {
            self.advance();
            Ok(name)
        } else {
            Err(self.error(format!("expected identifier, found {:?}", self.current())))
        }
    }

    fn recover(&mut self) {
        while !self.at_eof() {
            match self.current() {
                Tok::Shader | Tok::Surface | Tok::Displacement | Tok::Volume | Tok::Struct => {
                    return;
                }
                _ if self.is_type_keyword() => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn recover_to_next_statement(&mut self) {
        while !self.at_eof() {
            if self.at(&Tok::Semi) {
                self.advance();
                return;
            }
            if self.at(&Tok::RBrace) {
                return;
            }
            self.advance();
        }
    }
}

/// Parse result including both AST and any warnings.
pub struct ParseOutput {
    pub ast: Vec<ASTNode>,
    pub warnings: Vec<String>,
}

/// Convenience function: parse OSL source to AST.
pub fn parse(source: &str) -> Result<ParseOutput, Vec<ParseError>> {
    let mut parser = Parser::new(source);
    let result = parser.parse_shader_file();
    match result {
        Ok(nodes) if parser.errors.is_empty() => Ok(ParseOutput {
            ast: nodes,
            warnings: parser.warnings,
        }),
        Ok(_nodes) => Err(parser.errors),
        Err(e) => {
            parser.errors.push(e);
            Err(parser.errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_shader() {
        let src = r#"
surface simple_shader(float Kd = 0.5) {
    Ci = Kd;
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
        let nodes = result.unwrap().ast;
        assert_eq!(nodes.len(), 1);
        assert!(matches!(
            nodes[0].kind,
            ASTNodeKind::ShaderDeclaration { .. }
        ));
    }

    #[test]
    fn test_parse_expression() {
        let src = r#"
shader test(float a = 1.0, float b = 2.0) {
    float c = a + b * 3.0;
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_if_else() {
        let src = r#"
shader test(float x = 0.0) {
    if (x > 0.0) {
        x = 1.0;
    } else {
        x = -1.0;
    }
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_for_loop() {
        let src = r#"
shader test() {
    float sum = 0.0;
    for (int i = 0; i < 10; i++) {
        sum = sum + 1.0;
    }
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_function_call() {
        let src = r#"
shader test(color Cs = color(1, 0, 0)) {
    float l = luminance(Cs);
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_ternary() {
        let src = r#"
shader test(float a = 1.0) {
    float b = a > 0.5 ? 1.0 : 0.0;
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_type_constructor() {
        let src = r#"
shader test() {
    color c = color(0.5, 0.5, 0.5);
    point p = point(1, 2, 3);
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_compound_initializer() {
        let src = r#"
shader test() {
    float arr[3] = {1.0, 2.0, 3.0};
}
"#;
        let _result = parse(src);
    }

    #[test]
    fn test_parse_while_and_do() {
        let src = r#"
shader test() {
    int i = 0;
    while (i < 5) {
        i++;
    }
    do {
        i--;
    } while (i > 0);
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_function_declaration() {
        let src = r#"
float helper(float x) {
    return x * 2.0;
}

shader test() {
    float y = helper(3.0);
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_param_with_metadata() {
        // C++ formal_param: outputspec typespec IDENTIFIER initializer_opt metadata_block_opt
        let src = r#"
shader test(float Kd = 0.5 [[ int lockgeom = 0 ]]) {
    float x = Kd;
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_comma_operator_in_parens() {
        // (a, b, c) should create CommaOperator, NOT CompoundInitializer
        let src = r#"
shader test() {
    float x = 1.0;
    float y = 2.0;
    float z = 3.0;
    float w = (x, y, z);
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
        let nodes = result.unwrap().ast;
        // Walk into shader > statements > last var decl > init
        if let ASTNodeKind::ShaderDeclaration { statements, .. } = &nodes[0].kind
            && let ASTNodeKind::CompoundStatement { statements: stmts } = &statements[0].kind
        {
            // Find the last variable declaration (w = (x, y, z))
            let last = &stmts[stmts.len() - 1];
            if let ASTNodeKind::VariableDeclaration {
                init: Some(init_expr),
                ..
            } = &last.kind
            {
                assert!(
                    matches!(init_expr.kind, ASTNodeKind::CommaOperator { .. }),
                    "Expected CommaOperator, got {:?}",
                    init_expr.kind
                );
            } else {
                panic!("Expected VariableDeclaration with init");
            }
        }
    }

    #[test]
    fn test_compound_init_with_braces() {
        // {a, b, c} should remain CompoundInitializer
        let src = r#"
shader test() {
    float arr[3] = {1.0, 2.0, 3.0};
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_function_args_compound_init() {
        // Function args that are compound initializers in braces (C++ oslgram.y: f({1,2,3}))
        let src = r#"
shader test() {
    float x = max(1.0, 2.0);
    vector v = vector({1.0, 0.0, 0.0});
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_string_literal_concatenation() {
        // C++ string_literal_group: "hello" " world"
        let src = r#"
shader test() {
    string s = "hello" " " "world";
}
"#;
        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }
}
