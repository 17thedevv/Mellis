# Silent Fallback Audit (Section R)


### mellis-borrowck\src\effect.rs:26
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
            (_, AccessKind::None) => Some(std::cmp::Ordering::Greater),
            (AccessKind::Read, AccessKind::ReadWrite) => Some(std::cmp::Ordering::Less),
            (AccessKind::Write, AccessKind::ReadWrite) => Some(std::cmp::Ordering::Less),
            (AccessKind::ReadWrite, AccessKind::Read) => Some(std::cmp::Ordering::Greater),
            (AccessKind::ReadWrite, AccessKind::Write) => Some(std::cmp::Ordering::Greater),
            _ => None,
        }
    }
}

impl AccessKind {

```

### mellis-borrowck\src\effect.rs:109
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
                    Some(std::cmp::Ordering::Greater)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl ReturnEffect {

```

### mellis-borrowck\src\effect.rs:242
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
}

impl PartialOrd for CallEffectSummary {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.args.len() != other.args.len() {
            return None;
        }

        let mut has_less = false;
        let mut has_greater = false;


```

### mellis-borrowck\src\place.rs:34
**Fallback**: `if self.local != other.local { return false; }`
**Category**: UNSOUND (P0)

```rust
        self.projections.push(proj);
        self
    }
    
    pub fn is_ancestor_of(&self, other: &Place) -> bool {
        if self.local != other.local { return false; }
        if self.projections.len() > other.projections.len() { return false; }
        for (i, proj) in self.projections.iter().enumerate() {
            if proj != &other.projections[i] { return false; }
        }
        true

```

### mellis-borrowck\src\place.rs:35
**Fallback**: `if self.projections.len() > other.projections.len() { return false; }`
**Category**: UNSOUND (P0)

```rust
        self
    }
    
    pub fn is_ancestor_of(&self, other: &Place) -> bool {
        if self.local != other.local { return false; }
        if self.projections.len() > other.projections.len() { return false; }
        for (i, proj) in self.projections.iter().enumerate() {
            if proj != &other.projections[i] { return false; }
        }
        true
    }

```

### mellis-borrowck\src\place.rs:37
**Fallback**: `if proj != &other.projections[i] { return false; }`
**Category**: UNSOUND (P0)

```rust
    
    pub fn is_ancestor_of(&self, other: &Place) -> bool {
        if self.local != other.local { return false; }
        if self.projections.len() > other.projections.len() { return false; }
        for (i, proj) in self.projections.iter().enumerate() {
            if proj != &other.projections[i] { return false; }
        }
        true
    }
    
    pub fn is_descendant_of(&self, other: &Place) -> bool {

```

### mellis-common\src\source.rs:41
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
    }

    pub fn get_line_str(&self, line_idx: u32) -> Option<&str> {
        let idx = (line_idx - 1) as usize;
        if idx >= self.line_starts.len() {
            return None;
        }
        let start = self.line_starts[idx] as usize;
        let end = if idx + 1 < self.line_starts.len() {
            self.line_starts[idx + 1] as usize - 1 // Exclude \n
        } else {

```

### mellis-lexer\src\lexer.rs:47
**Fallback**: `return 0;`
**Category**: UNSOUND (P0)

```rust
        }
    }

    fn advance(&mut self) -> u8 {
        if self.is_at_end() {
            return 0;
        }
        let c = self.bytes[self.pos];
        self.pos += 1;
        c
    }

```

### mellis-lexer\src\lexer.rs:347
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust

        let start_offset = self.pos;
        if self.is_at_end() {
            // Để Lexer dừng lại hẳn sau khi báo Eof lần 1, ta có thể lưu 1 cờ.
            // Nhưng hiện tại Iterator trả về None khi hết.
            return None;
        }

        let c = self.advance();

        if c.is_ascii_alphabetic() || c == b'_' {

```

### mellis-mlib\src\format.rs:115
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
            12 => Some(SectionType::MacroMetadata),
            13 => Some(SectionType::GenericMetadata),
            14 => Some(SectionType::TypeRefTable),
            15 => Some(SectionType::AstInterface),
            0xFFFFFFFF => Some(SectionType::Custom),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]

```

### mellis-mvir\src\generator.rs:1368
**Fallback**: `_ => None,`
**Category**: SAFE (Diagnostic/Span)

```rust
            mellis_ast::Expr::Assign { lvalue, .. } => self.extract_expr_span(lvalue),
            mellis_ast::Expr::Call { callee, .. } => self.extract_expr_span(callee),
            mellis_ast::Expr::Index { base, .. } => self.extract_expr_span(base),
            mellis_ast::Expr::TupleIndex { object, .. } => self.extract_expr_span(object),
            mellis_ast::Expr::Cast { expr, .. } => self.extract_expr_span(expr),
            _ => None,
        }
    }

    fn push_inst_span(&mut self, inst: Instruction, ty: mellis_semantic::SemanticTypeId, span: Option<mellis_common::Span>) -> ValueId {
        let func = self.current_function.as_mut().expect("Must be in a function");

```

### mellis-mvir\src\interp.rs:172
**Fallback**: `_ => false,`
**Category**: INTERNAL RECOVERY / UNSOUND

```rust
                matches!(mutability, mellis_semantic::ty::Mutability::Immutable)
            }
            SemanticType::Pointer(..) => true,
            SemanticType::Tuple(elems) => elems.iter().all(|&e| self.is_copy_type(e)),
            SemanticType::Array(elem, _) => self.is_copy_type(*elem),
            _ => false,
        }
    }

    fn get_slot_mut(&mut self, addr: Address) -> Result<&mut MemorySlot, ComptimeError> {
        match addr {

```

### mellis-optimizer\src\passes\const_fold.rs:56
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
                Instruction::NotEq { left, right } => {
                    if let (Some(l), Some(r)) = (Self::resolve_const(func, left), Self::resolve_const(func, right)) {
                        Some(Instruction::Assign(Operand::Boolean(l != r)))
                    } else { None }
                }
                _ => None,
            };

            if let Some(inst) = new_inst {
                func.values[i].inst = inst;
                changed = true;

```

### mellis-optimizer\src\passes\const_fold.rs:82
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
            Operand::Number(n) => n.parse::<i64>().ok(),
            Operand::Value(val_id) => {
                let def = func.value(*val_id);
                match &def.inst {
                    Instruction::Assign(Operand::Number(n)) => n.parse::<i64>().ok(),
                    _ => None,
                }
            }
            _ => None,
        }
    }

```

### mellis-optimizer\src\passes\const_fold.rs:85
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
                match &def.inst {
                    Instruction::Assign(Operand::Number(n)) => n.parse::<i64>().ok(),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

```

### mellis-parser\src\expr.rs:9
**Fallback**: `return false;`
**Category**: UNSOUND (P0)

```rust
use mellis_lexer::TokenKind;

impl<'a> Parser<'a> {
    fn is_value_generic_args(&self) -> bool {
        if !self.check(TokenKind::LessThan) {
            return false;
        }
        let mut p = self.pos + 1;
        let mut depth = 1;
        while p < self.tokens.len() {
            let kind = self.tokens[p].kind;

```

### mellis-parser\src\expr.rs:24
**Fallback**: `return false;`
**Category**: UNSOUND (P0)

```rust
                if depth == 0 {
                    let next_kind = self.tokens.get(p + 1).map(|t| t.kind).unwrap_or(TokenKind::Eof);
                    return matches!(next_kind, TokenKind::ColonColon | TokenKind::LParen | TokenKind::LBrace);
                }
            } else if kind == TokenKind::Eof || kind == TokenKind::Semi {
                return false;
            }
            p += 1;
        }
        false
    }

```

### mellis-parser\src\expr.rs:54
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
            TokenKind::BitAndAssign => Some(AssignOp::BitAndAssign),
            TokenKind::BitOrAssign => Some(AssignOp::BitOrAssign),
            TokenKind::BitXorAssign => Some(AssignOp::BitXorAssign),
            TokenKind::LShiftAssign => Some(AssignOp::LShiftAssign),
            TokenKind::RShiftAssign => Some(AssignOp::RShiftAssign),
            _ => None,
        };

        if let Some(assign_op) = op {
            self.advance();
            let value = self.parse_assignment(allow_struct_literal)?;

```

### mellis-semantic\src\annotation.rs:389
**Fallback**: `_ => None,`
**Category**: SAFE (Diagnostic/Span)

```rust
    /// Get the span for an expression.
    fn get_expr_span(&self, expr_id: mellis_ast::ExprId) -> Option<Span> {
        match &self.arena.exprs[expr_id.0 as usize] {
            Expr::Identifier { segments, .. } => segments.first().copied(),
            Expr::Literal(tok) => Some(tok.span),
            _ => None,
        }
    }
}

#[cfg(test)]

```

### mellis-semantic\src\derive.rs:404
**Fallback**: `match_arms.push(format!("{}::{} -> {{ return match *other {{ {}::{} -> {{ return true; }} _ -> {{ return false; }} }}; }}",`
**Category**: UNSOUND (P0)

```rust
                }
                DeriveKind::Enum => {
                    let mut match_arms = Vec::new();
                    for variant in &input.variants {
                        if variant.fields.is_empty() {
                            match_arms.push(format!("{}::{} -> {{ return match *other {{ {}::{} -> {{ return true; }} _ -> {{ return false; }} }}; }}", 
                                type_name, variant.name, type_name, variant.name));
                        } else {
                            let self_binders: Vec<String> = variant.fields.iter().enumerate().map(|(i, _)| format!("s{}", i)).collect();
                            let other_binders: Vec<String> = variant.fields.iter().enumerate().map(|(i, _)| format!("o{}", i)).collect();
                            let comparisons: Vec<String> = self_binders.iter().zip(other_binders.iter())

```

### mellis-semantic\src\derive.rs:413
**Fallback**: `match_arms.push(format!("{}::{}({}) -> {{ return match *other {{ {}::{}({}) -> {{ return {}; }} _ -> {{ return false; }} }}; }}",`
**Category**: UNSOUND (P0)

```rust
                            let other_binders: Vec<String> = variant.fields.iter().enumerate().map(|(i, _)| format!("o{}", i)).collect();
                            let comparisons: Vec<String> = self_binders.iter().zip(other_binders.iter())
                                .map(|(s, o)| format!("{}.eq(&{})", s, o))
                                .collect();
                            let cond = comparisons.join(" && ");
                            match_arms.push(format!("{}::{}({}) -> {{ return match *other {{ {}::{}({}) -> {{ return {}; }} _ -> {{ return false; }} }}; }}", 
                                type_name, variant.name, self_binders.join(", "),
                                type_name, variant.name, other_binders.join(", "),
                                cond));
                        }
                    }

```

### mellis-semantic\src\derive.rs:420
**Fallback**: `"impl PartialEq for {} {{ fn eq(self: &{}, other: &{}) -> bool {{ return match *self {{ {} _ -> {{ return false; }} }}; }} }}",`
**Category**: UNSOUND (P0)

```rust
                                type_name, variant.name, other_binders.join(", "),
                                cond));
                        }
                    }
                    format!(
                        "impl PartialEq for {} {{ fn eq(self: &{}, other: &{}) -> bool {{ return match *self {{ {} _ -> {{ return false; }} }}; }} }}",
                        type_name, type_name, type_name, match_arms.join(" ")
                    )
                }
            };


```

### mellis-semantic\src\derive.rs:581
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
                fields: field_infos,
                variants: Vec::new(),
                annotations: annots,
            })
        }
        _ => None,
    }
}

/// Extract DeriveInput from an enum declaration.
pub fn extract_enum_input(arena: &AstArena, source: &str, decl: &Decl) -> Option<DeriveInput> {

```

### mellis-semantic\src\derive.rs:636
**Fallback**: `_ => None,`
**Category**: SAFE (Diagnostic/Span)

```rust
                fields: Vec::new(),
                variants: variant_infos,
                annotations: annots,
            })
        }
        _ => None,
    }
}

fn get_span_text(source: &str, span: Span) -> String {
    if (span.end as usize) <= source.len() && span.start <= span.end {

```

### mellis-semantic\src\lib.rs:124
**Fallback**: `_ => false,`
**Category**: INTERNAL RECOVERY / UNSOUND

```rust
                self.needs_drop(*elem_ty)
            }
            ty::SemanticType::Future(inner) => {
                self.needs_drop(*inner)
            }
            _ => false,
        };
        
        self.needs_drop_cache.borrow_mut().insert(id, if result { NeedsDropState::Yes } else { NeedsDropState::No });
        result
    }

```

### mellis-semantic\src\macro_engine.rs:599
**Fallback**: `return None;`
**Category**: SAFE (Diagnostic/Span)

```rust
                    "recursion limit reached while expanding macro `{}`",
                    macro_name
                ))
                .with_span(call_span),
            );
            return None;
        }

        self.recursion_depth += 1;
        self.expansion_counter += 1;
        let expansion_id = self.expansion_counter;

```

### mellis-semantic\src\macro_engine.rs:662
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
            if let Some(consumed) = self.match_pattern_elements(tokens, 0, &rule.pattern.elements, &mut captures) {
                if consumed == tokens.len() {
                    return Some(captures);
                }
            }
            return None;
        }

        // Fallback for flat matchers
        self.match_flat_rule(tokens, &rule.matchers)
    }

```

### mellis-semantic\src\macro_engine.rs:680
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
    ) -> Option<usize> {
        for element in elements {
            match element {
                MatcherElement::Leaf { token: pat_tok } => {
                    if pos >= tokens.len() {
                        return None;
                    }
                    if tokens[pos].kind != pat_tok.kind {
                        return None;
                    }
                    pos += 1;

```

### mellis-semantic\src\macro_engine.rs:683
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
                MatcherElement::Leaf { token: pat_tok } => {
                    if pos >= tokens.len() {
                        return None;
                    }
                    if tokens[pos].kind != pat_tok.kind {
                        return None;
                    }
                    pos += 1;
                }
                MatcherElement::Group { delimiter, elements: group_elements, .. } => {
                    if pos >= tokens.len() {

```

### mellis-semantic\src\macro_engine.rs:689
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
                    }
                    pos += 1;
                }
                MatcherElement::Group { delimiter, elements: group_elements, .. } => {
                    if pos >= tokens.len() {
                        return None;
                    }
                    let (open_kind, close_kind) = match delimiter {
                        MacroDelimiter::Paren => (TokenKind::LParen, TokenKind::RParen),
                        MacroDelimiter::Bracket => (TokenKind::LBracket, TokenKind::RBracket),
                        MacroDelimiter::Brace => (TokenKind::LBrace, TokenKind::RBrace),

```

### mellis-semantic\src\macro_engine.rs:698
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
                        MacroDelimiter::Bracket => (TokenKind::LBracket, TokenKind::RBracket),
                        MacroDelimiter::Brace => (TokenKind::LBrace, TokenKind::RBrace),
                    };

                    if tokens[pos].kind != open_kind {
                        return None;
                    }
                    pos += 1;

                    let group_start = pos;
                    let mut depth = 1;

```

### mellis-semantic\src\macro_engine.rs:717
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
                        }
                        pos += 1;
                    }

                    if depth != 0 || pos >= tokens.len() {
                        return None;
                    }

                    let inner_tokens = &tokens[group_start..pos];
                    pos += 1; // consume close_kind


```

### mellis-semantic\src\macro_engine.rs:725
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
                    let inner_tokens = &tokens[group_start..pos];
                    pos += 1; // consume close_kind

                    if let Some(inner_consumed) = self.match_pattern_elements(inner_tokens, 0, group_elements, captures) {
                        if inner_consumed != inner_tokens.len() {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }

```

### mellis-semantic\src\macro_engine.rs:728
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
                    if let Some(inner_consumed) = self.match_pattern_elements(inner_tokens, 0, group_elements, captures) {
                        if inner_consumed != inner_tokens.len() {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                MatcherElement::MetaVar { name, fragment, .. } => {
                    if pos >= tokens.len() {
                        return None;

```

### mellis-semantic\src\macro_engine.rs:733
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
                        return None;
                    }
                }
                MatcherElement::MetaVar { name, fragment, .. } => {
                    if pos >= tokens.len() {
                        return None;
                    }
                    let var_name = self.source[name.start as usize..name.end as usize].to_string();
                    let consumed = self.capture_fragment_parser(tokens, pos, *fragment)?;
                    if consumed == 0 {
                        return None;

```

### mellis-semantic\src\macro_engine.rs:738
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
                        return None;
                    }
                    let var_name = self.source[name.start as usize..name.end as usize].to_string();
                    let consumed = self.capture_fragment_parser(tokens, pos, *fragment)?;
                    if consumed == 0 {
                        return None;
                    }
                    let captured_tokens = tokens[pos..pos + consumed].to_vec();
                    captures.insert(var_name, CapturedFragment::Single(captured_tokens));
                    pos += consumed;
                }

```

### mellis-semantic\src\macro_engine.rs:745
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
                    let captured_tokens = tokens[pos..pos + consumed].to_vec();
                    captures.insert(var_name, CapturedFragment::Single(captured_tokens));
                    pos += consumed;
                }
                MatcherElement::Repetition { .. } => {
                    return None;
                }
            }
        }
        Some(pos)
    }

```

### mellis-semantic\src\macro_engine.rs:759
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
        tokens: &[Token],
        pos: usize,
        fragment: FragmentKind,
    ) -> Option<usize> {
        if pos >= tokens.len() {
            return None;
        }

        match fragment {
            FragmentKind::Ident => {
                let tok = tokens[pos];

```

### mellis-semantic\src\macro_engine.rs:777
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, self.source, &mut scratch_arena, self.file_id);
                match parser.parse_expression(true) {
                    Ok(_) if parser.pos() > 0 => Some(parser.pos()),
                    _ => None,
                }
            }
            FragmentKind::Ty => {
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();

```

### mellis-semantic\src\macro_engine.rs:786
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, self.source, &mut scratch_arena, self.file_id);
                match parser.parse_type() {
                    Ok(_) if parser.pos() > 0 => Some(parser.pos()),
                    _ => None,
                }
            }
            FragmentKind::Stmt => {
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();

```

### mellis-semantic\src\macro_engine.rs:795
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, self.source, &mut scratch_arena, self.file_id);
                match parser.parse_stmt() {
                    Ok(_) if parser.pos() > 0 => Some(parser.pos()),
                    _ => None,
                }
            }
            FragmentKind::Block => {
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();

```

### mellis-semantic\src\macro_engine.rs:804
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, self.source, &mut scratch_arena, self.file_id);
                match parser.parse_block_stmt() {
                    Ok(_) if parser.pos() > 0 => Some(parser.pos()),
                    _ => None,
                }
            }
            FragmentKind::Item => {
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();

```

### mellis-semantic\src\macro_engine.rs:813
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, self.source, &mut scratch_arena, self.file_id);
                match parser.parse_item() {
                    Ok(_) if parser.pos() > 0 => Some(parser.pos()),
                    _ => None,
                }
            }
        }
    }


```

### mellis-semantic\src\macro_engine.rs:927
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
                        }
                    }
                }

                if rep_kind == RepetitionKind::OneOrMore && repeated_tokens.is_empty() {
                    return None;
                }

                captures.insert(var_name, CapturedFragment::Repeated(repeated_tokens));
            } else {
                let frag_tokens = self.capture_one_fragment(tokens, &mut pos, matcher.fragment, Some(TokenKind::Comma), is_last)?;

```

### mellis-semantic\src\macro_engine.rs:939
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust

                if !is_last {
                    if pos < tokens.len() && tokens[pos].kind == TokenKind::Comma {
                        pos += 1;
                    } else {
                        return None;
                    }
                }
            }
        }


```

### mellis-semantic\src\macro_engine.rs:961
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
        fragment: FragmentKind,
        stop_sep: Option<TokenKind>,
        _is_last: bool,
    ) -> Option<Vec<Token>> {
        if *pos >= tokens.len() {
            return None;
        }

        match fragment {
            FragmentKind::Ident => {
                if tokens[*pos].kind == TokenKind::Identifier || tokens[*pos].kind == TokenKind::KwSelfVal {

```

### mellis-semantic\src\symbol.rs:246
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
        self.lookup_macro_with_ctxt(path, SyntaxContext::ROOT, start_scope)
    }

    pub fn lookup_macro_with_ctxt(&self, path: &[&str], ctxt: SyntaxContext, start_scope: ScopeId) -> Option<SymbolId> {
        if path.is_empty() {
            return None;
        }
        if path.len() == 1 {
            if let Some(sym_id) = self.lookup_with_ctxt(path[0], ctxt, start_scope) {
                if matches!(self.symbols[sym_id.0 as usize].kind, SymbolKind::Macro) {
                    return Some(sym_id);

```

### mellis-semantic\src\symbol.rs:254
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
            if let Some(sym_id) = self.lookup_with_ctxt(path[0], ctxt, start_scope) {
                if matches!(self.symbols[sym_id.0 as usize].kind, SymbolKind::Macro) {
                    return Some(sym_id);
                }
            }
            return None;
        }

        // Qualified path: e.g. ["module", "macro_name"] or ["alias", "macro_name"]
        let first_seg = path[0];
        let mut current_scope = if let Some(sym_id) = self.lookup_with_ctxt(first_seg, ctxt, start_scope) {

```

### mellis-semantic\src\symbol.rs:263
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
        let first_seg = path[0];
        let mut current_scope = if let Some(sym_id) = self.lookup_with_ctxt(first_seg, ctxt, start_scope) {
            let sym = &self.symbols[sym_id.0 as usize];
            sym.inner_scope?
        } else {
            return None;
        };

        for &seg in &path[1..path.len() - 1] {
            if let Some(sym_id) = self.lookup_exact_with_ctxt(seg, ctxt, current_scope) {
                let sym = &self.symbols[sym_id.0 as usize];

```

### mellis-semantic\src\symbol.rs:272
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
            if let Some(sym_id) = self.lookup_exact_with_ctxt(seg, ctxt, current_scope) {
                let sym = &self.symbols[sym_id.0 as usize];
                if let Some(inner) = sym.inner_scope {
                    current_scope = inner;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }

```

### mellis-semantic\src\symbol.rs:275
**Fallback**: `return None;`
**Category**: UNSOUND (P0)

```rust
                    current_scope = inner;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }

        let last_seg = path[path.len() - 1];
        if let Some(sym_id) = self.lookup_exact_with_ctxt(last_seg, ctxt, current_scope) {

```

### mellis-semantic\src\symbol.rs:310
**Fallback**: `return false;`
**Category**: UNSOUND (P0)

```rust
                return true;
            }
            if let Some(p) = self.scopes[child.0 as usize].parent {
                child = p;
            } else {
                return false;
            }
        }
    }

    pub fn get_symbol(&self, id: SymbolId) -> &Symbol {

```

### mellis-semantic\src\typechecker.rs:538
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
                                Some(*s)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    if let (Some(concrete_sym), Some(bounds)) = (concrete_struct_sym, self.ctx.tables.trait_bounds.get(&gp_sym).cloned()) {
                        for bound in &bounds {
                            let impl_key = crate::semantic_tables::ImplKey {
                                trait_id: Some(bound.trait_id),

```

### mellis-semantic\src\typechecker.rs:621
**Fallback**: `return false;`
**Category**: SAFE (Diagnostic/Span)

```rust
                        self.ctx.symbol_table.get_symbol(trait_sym).name,
                        method_name
                    ));
                    if let Some(sp) = span { diag = diag.with_span(sp); }
                    self.ctx.diagnostics.push(diag);
                    return false;
                }
                
                // Rule 2: Must have receiver (`self`, `&self`, or `&rw self`)
                if params.is_empty() {
                    let mut diag = Diagnostic::error(format!(

```

### mellis-semantic\src\typechecker.rs:633
**Fallback**: `return false;`
**Category**: SAFE (Diagnostic/Span)

```rust
                        self.ctx.symbol_table.get_symbol(trait_sym).name,
                        method_name
                    ));
                    if let Some(sp) = span { diag = diag.with_span(sp); }
                    self.ctx.diagnostics.push(diag);
                    return false;
                }
                let first_param = &self.arena.decls[params[0].0 as usize];
                if let Decl::Param { is_self, .. } = first_param {
                    if !*is_self {
                        let mut diag = Diagnostic::error(format!(

```

### mellis-semantic\src\typechecker.rs:645
**Fallback**: `return false;`
**Category**: SAFE (Diagnostic/Span)

```rust
                            self.ctx.symbol_table.get_symbol(trait_sym).name,
                            method_name
                        ));
                        if let Some(sp) = span { diag = diag.with_span(sp); }
                        self.ctx.diagnostics.push(diag);
                        return false;
                    }
                }
                
                // Rule 3: Return type cannot be unboxed `Self`
                if let Some(ret_ty_id) = return_type {

```

### mellis-semantic\src\typechecker.rs:663
**Fallback**: `return false;`
**Category**: SAFE (Diagnostic/Span)

```rust
                                    self.ctx.symbol_table.get_symbol(trait_sym).name,
                                    method_name
                                ));
                                if let Some(sp) = span { diag = diag.with_span(sp); }
                                self.ctx.diagnostics.push(diag);
                                return false;
                            }
                        }
                    }
                }
            }

```

### mellis-semantic\src\typechecker.rs:686
**Fallback**: `return false;`
**Category**: UNSOUND (P0)

```rust
            | (SemanticType::Reference(_, _, f_in), SemanticType::Pointer(_, t_in)) => {
                let t_ty = self.ctx.types.get(*t_in).clone();
                if let SemanticType::DynTrait(trait_sym) = t_ty {
                    (*f_in, trait_sym)
                } else {
                    return false;
                }
            }
            (_, SemanticType::DynTrait(trait_sym)) => {
                (from_ty, *trait_sym)
            }

```

### mellis-semantic\src\typechecker.rs:1083
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
            }
            Stmt::Return { value: Some(expr) } => self.ctx.tables.expr_types.get(expr).copied(),
            Stmt::If { then_branch, else_branch: Some(else_branch), .. } => {
                self.infer_stmt_value_type(then_branch).or_else(|| self.infer_stmt_value_type(else_branch))
            }
            _ => None,
        }
    }

    fn typecheck_stmt(&mut self, stmt_id: &mellis_ast::StmtId) {
        let stmt = &self.arena.stmts[stmt_id.0 as usize];

```

### mellis-semantic\src\typechecker.rs:1166
**Fallback**: `_ => SemanticType::Error,`
**Category**: UNSOUND (P0)

```rust
                    TokenKind::IntegerLiteral => SemanticType::Primitive(BuiltinType::I32), // Default to i32
                    TokenKind::FloatLiteral => SemanticType::Primitive(BuiltinType::F64), // Default to f64
                    TokenKind::StringLiteral => SemanticType::Primitive(BuiltinType::String),
                    TokenKind::CharLiteral => SemanticType::Primitive(BuiltinType::Char),
                    TokenKind::KwTrue | TokenKind::KwFalse => SemanticType::Primitive(BuiltinType::Bool),
                    _ => SemanticType::Error,
                };
                self.ctx.types.intern(kind)
            }
            Expr::Identifier { .. } => {
                if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id) {

```

### mellis-semantic\src\typechecker.rs:1333
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
                            Some(*sym)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(trait_sym) = dyn_trait_sym {
                    if let Some(method_syms) = self.ctx.tables.trait_methods.get(&trait_sym) {
                        for (idx, &m_sym) in method_syms.iter().enumerate() {
                            let sym = self.ctx.symbol_table.get_symbol(m_sym);

```

### mellis-semantic\src\typechecker.rs:1508
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
                            Some(*sym)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(trait_sym) = dyn_trait_sym {
                    if let Some(method_syms) = self.ctx.tables.trait_methods.get(&trait_sym) {
                        for (idx, &m_sym) in method_syms.iter().enumerate() {
                            let sym = self.ctx.symbol_table.get_symbol(m_sym);

```

### mellis-semantic\src\typechecker.rs:1785
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
                        env_tys.push(env_ty);
                        let is_mutated = self.ctx.tables.closure_mutated_captures.get(expr_id).map_or(false, |mutated| mutated.contains(&sym_id));
                        let is_mutable = self.ctx.tables.symbol_decls.get(&sym_id).and_then(|decl_id| match &self.arena.decls[decl_id.0 as usize] {
                            Decl::Var { is_mutable, .. } => Some(*is_mutable),
                            Decl::Param { .. } => Some(true),
                            _ => None,
                        }).unwrap_or(false);
                        if is_mutated && !is_mutable {
                            self.ctx.diagnostics.push(Diagnostic::error("Cannot mutably capture immutable variable"));
                        }
                        capture_bindings.push(crate::semantic_tables::CaptureBinding {

```

### mellis-semantic\src\typechecker.rs:1887
**Fallback**: `_ => None,`
**Category**: SAFE (Diagnostic/Span)

```rust
            Expr::Literal(tok) => Some(tok.span),
            Expr::Identifier { segments, .. } => segments.first().copied(),
            Expr::Call { callee, .. } => self.get_expr_span_for_diag(callee),
            Expr::MethodCall { method_name, .. } => Some(*method_name),
            Expr::Await { expr } => self.get_expr_span_for_diag(expr),
            _ => None,
        }
    }

    fn enforce_mutability(&mut self, expr_id: &mellis_ast::ExprId) {
        let expr = &self.arena.exprs[expr_id.0 as usize];

```

### mellis-semantic\src\comptime\value.rs:150
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
    }

    pub fn as_i128(&self) -> Option<i128> {
        match self {
            ComptimeValue::Int { val, .. } => Some(*val),
            _ => None,
        }
    }

    pub fn as_usize(&self) -> Option<usize> {
        match self {

```

### mellis-semantic\src\comptime\value.rs:157
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
    }

    pub fn as_usize(&self) -> Option<usize> {
        match self {
            ComptimeValue::Int { val, .. } if *val >= 0 => Some(*val as usize),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {

```

### mellis-semantic\src\comptime\value.rs:164
**Fallback**: `_ => None,`
**Category**: UNSOUND (P0)

```rust
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ComptimeValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn add(&self, other: &Self) -> Result<Self, ComptimeError> {
        match (self, other) {

```
