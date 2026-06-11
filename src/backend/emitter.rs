use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::cell::{Cell/*, RefCell*/};

use crate::frontend::{
    token::{TokenKind, Token},
    ast::{Operator, SyntaxId, SyntaxData, SyntaxNode, AST},
};

use crate::runtime::code::InlineCache;
use crate::runtime::{
    code::{Opcode, Instruction, Chunk, Program},
    values::{JSValue},
    objects::{JSObjPtr, JSStrPtr, JS_OBJECT_COST, JS_STRING_COST, ItemPool, /*ExoticObject*/},
    // funcs::{FuncBody, JSFunction},
};


pub const JS_BOOLEAN_PROTO_ALIAS: &str = "[[Array.prototype]]";
pub const JS_NUMBER_PROTO_ALIAS: &str = "[[Array.prototype]]";
pub const JS_OBJECT_PROTO_ALIAS: &str = "[[Object.prototype]]";
pub const JS_ARRAY_PROTO_ALIAS: &str = "[[Array.prototype]]";
pub const JS_FUNC_PROTO_ALIAS: &str = "[[Function.prototype]]";


#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SymbolTag {
    /// Chunk-local constant
    Constant,
    /// Globally available / stored object
    GlobalObj,
    GlobalStr,
    /// Globally available property key string
    KeyStr,
    /// Slot ID of simple function variable (not a property & not a captured or capturing name)
    Local,
}

#[derive(Debug, Clone, Copy)]
pub struct SymbolInfo {
    pub id: i32,
    pub tag: SymbolTag,
}

pub struct SymbolScope {
    /// Maps symbol strings to a piece of info
    pub symbols: HashMap<String, SymbolInfo>,
    pub next_local_id: i32, // ? calculated in any stmt block prepass, which helps elucidate how many stack slots are needed for all vars / lets
    pub next_const_id: i32,
    pub next_ic_id: u8,
}

impl Default for SymbolScope {
    fn default() -> Self {
        Self {
            symbols: HashMap::default(),
            next_local_id: 1,
            next_const_id: 0,
            next_ic_id: 0,
        }
    }
}

#[repr(u16)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EmitterFlag {
    /// The emitter should analyze the sub-AST for anything captured from caller / by possible callees.
    CheckFnIsSimple = (1 << 0),
    /// The emitter should visit the sub-AST to resolve hoisted names.
    PrepassVars = (1 << 1),
    /// Is the expression at global scope / top-level?
    InTopLevel = (1 << 2),
    /// Indicates if an expression is a destination for side effects i.e assignments, adding/deleting properties, etc.
    InLocator = (1 << 3),
    /// Indicates if an expression is inside a LHS expression.
    InAccess = (1 << 4),
    /// Indicates if an expression is somewhere within a `new` expression.
    InConstruction = (1 << 5),
    /// Are the function's variable semantics simple, specifically not requiring an environment object?
    IsFuncSimple = (1 << 6),
    /// Does the bytecode generation have to consider `++target` or `target++`?
    HandleUnarySpecials = (1 << 7),
    /// Has the sub-AST compilation succeeded?
    IsVisitOK = (1 << 8),
}

#[derive(Default, Clone, Copy)]
pub struct EmitterHints {
    pub bits: u16
}

impl EmitterHints {
    pub fn enable_flag(&mut self, bitflag: EmitterFlag) {
        self.bits |= bitflag as u16;
    }

    pub fn disable_flag(&mut self, bitflag: EmitterFlag) {
        self.bits &= !(bitflag as u16);
    }

    pub fn get_flag(&self, bitflag: EmitterFlag) -> bool {
        (self.bits & (bitflag as u16)) == bitflag as u16
    }

    pub fn with_flag(mut self, bitflag: EmitterFlag) -> Self {
        self.enable_flag(bitflag);
        self
    }

    pub fn without_flag(mut self, bitflag: EmitterFlag) -> Self {
        self.disable_flag(bitflag);
        self
    }

    pub fn check_ok(&self) -> bool {
        self.get_flag(EmitterFlag::IsVisitOK)
    }
}

pub struct Emitter {
    pub heap: ItemPool<JSObjPtr, JS_OBJECT_COST>,
    pub spool: ItemPool<JSStrPtr, JS_STRING_COST>,
    pub capturing_names: HashSet<String>,
    pub chunks: Vec<Chunk>,
    pub scopes: Vec<SymbolScope>,
    pub cached_info: Option<SymbolInfo>,
    pub local_reserve_n: i32,
    pub line: u16,
}

impl Emitter {
    pub fn new(object_population: usize, str_population: usize) -> Self {
        Self {
            heap: ItemPool::<JSObjPtr, JS_OBJECT_COST>::new(object_population),
            spool: ItemPool::<JSStrPtr, JS_STRING_COST>::new(str_population),
            capturing_names: HashSet::default(),
            chunks: vec![Chunk {
                icaches: vec![],
                consts: vec![],
                code: vec![]
            }],
            scopes: vec![SymbolScope::default()],
            cached_info: None,
            local_reserve_n: 0,
            line: 1,
        }
    }

    fn put_ic_of_inst(&mut self, inst_pos: i32) {
        let next_ic_id = self.chunks.last().expect("Expected available chunk at emitter.rs ~ line#154.").icaches.len() as u16;

        self.chunks.last_mut().unwrap().icaches.push(InlineCache::default());
        self.chunks.last_mut().unwrap().code[inst_pos as usize].flags |= next_ic_id;
    }

    fn emit_nonary_inst(&mut self, op: Opcode, flags: u16) -> i32 {
        let curr_chunk = self.chunks.last_mut().expect("Expected available chunk in emitter.rs ~ line#160.");
        let curr_ip = curr_chunk.code.len() as i32;

        curr_chunk.code.push(Instruction { arg: 0, flags, op });

        curr_ip
    }

    fn emit_unary_inst(&mut self, op: Opcode, arg: i32, flags: u16) -> i32 {
        let curr_chunk = self.chunks.last_mut().expect("Expected current chunk as available in Emitter::emit_unary_inst!");
        let curr_ip = curr_chunk.code.len() as i32;

        curr_chunk.code.push(Instruction { arg, flags, op });

        curr_ip
    }

    fn resolve_global(&mut self, s: &str) -> Option<SymbolInfo> {
        let global_scope = self.scopes.first().expect("Expected global scope in Emitter::resolve_global()!");

        if let Some(info) = global_scope.symbols.get(s).copied() && matches!(info.tag, SymbolTag::GlobalStr | SymbolTag::GlobalObj) {
            return Some(info);
        }

        None
    }

    fn resolve_local(&mut self, s: &str) -> Option<SymbolInfo> {
        let current_scope = self.scopes.last().expect("Expected a symbol scope in Emitter::resolve_local()!");

        if let Some(info) = current_scope.symbols.get(s).copied() && info.tag == SymbolTag::Local {
            return Some(info);
        }

        None
    }

    fn resolve_global_string(&mut self, s: &str, is_key: bool) -> Option<SymbolInfo> {
        let global_scope = self.scopes.first().expect("Expected global scope in Emitter::resolve_string_constant()!");

        let (expected_tag, expected_name) = if is_key {
            (SymbolTag::KeyStr, format!("[[{s}]]"))
        } else {
            (SymbolTag::GlobalStr, s.to_owned())
        };

        if let Some(info) = global_scope.symbols.get(
            &expected_name
        ).copied() && expected_tag == info.tag {
            return Some(info);
        }

        None
    }

    fn resolve_constant(&self, s: &str) -> Option<SymbolInfo> {
        let current_scope = self.scopes.last().expect("Expected pushed scope at Emitter::resolve_constant()!");

        if let Some(info) = current_scope.symbols.get(s).copied() && info.tag == SymbolTag::Constant {
            return Some(info);
        }

        None
    }

    /// This is used to record built-in global objects e.g ctors of Array and Object, console, etc. 
    #[allow(unused)]
    fn record_global_object(&mut self, s: &str, object: JSObjPtr) -> Option<SymbolInfo> {
        if let Some(oid) = self.heap.add_item(object) {
            let temp_info = SymbolInfo {
                id: oid,
                tag: SymbolTag::GlobalObj,
            };

            self.scopes.first_mut().expect("Expected global scope to be tracked in Emitter::record_global_object()!").symbols.insert(s.to_owned(), temp_info);

            return Some(temp_info);
        }

        None
    }

    fn record_global_string(&mut self, s: &str, is_key: bool) -> Option<SymbolInfo> {
        if let Some(pre_info) = self.resolve_global_string(s, is_key) {
            return Some(pre_info);
        }
        
        let real_string_symbol = if is_key { format!("[[{s}]]") } else { s.to_owned() };
        let real_string = s.to_owned();
        let real_string: JSStrPtr = Some(Rc::new(Cell::new(
            real_string
        )));

        if let Some(sid) = self.spool.add_item(real_string) {
            let temp_info = SymbolInfo {
                id: sid,
                tag: if is_key {SymbolTag::KeyStr} else {SymbolTag::GlobalStr},
            };


            self.scopes.first_mut().expect("Expected global scope to be tracked in Emitter::record_global_string()!").symbols.insert(real_string_symbol, temp_info);

            return Some(temp_info);
        }

        None
    }

    fn record_local(&mut self, s: &str) -> Option<SymbolInfo> {
        let current_scope = self.scopes.last_mut().expect("Expected tracked symbol scope in Emitter::record_local()!");

        if let Some(pre_info) = current_scope.symbols.get(s) {
            Some(*pre_info)
        } else {
            let next_local_id = current_scope.next_local_id;
            let temp_info = SymbolInfo {
                id: next_local_id,
                tag: SymbolTag::Local,
            };

            current_scope.symbols.insert(s.to_owned(), temp_info);
            current_scope.next_local_id += 1;

            Some(temp_info)
        }
    }

    fn record_constant(&mut self, s: &str, v: JSValue) -> Option<SymbolInfo> {
        if let Some(pre_info) = self.resolve_constant(s) {
            Some(pre_info)
        } else {
            let current_scope = self.scopes.last_mut().expect("Expected tracked symbol scope in Emitter::record_local()!");
            let next_local_id = current_scope.next_const_id;
            let temp_info = SymbolInfo {
                id: next_local_id,
                tag: SymbolTag::Constant,
            };

            current_scope.symbols.insert(s.to_owned(), temp_info);
            current_scope.next_const_id += 1;
            self.chunks.last_mut().expect("Expected available chunk for Emitter::record_constant()!").consts.push(v);

            Some(temp_info)
        }
    }

    fn emit_name(&mut self, lexeme: &str, hints: EmitterHints) -> EmitterHints {
        // ? 1. Handle globalThis var bindings like `globalThisEnv.[[name]]`
        // ? 2. Handle simple, non capturing funcs

        let name_in_locator = hints.get_flag(EmitterFlag::InLocator);
        let name_in_access = hints.get_flag(EmitterFlag::InAccess);
        let name_lacks_env = hints.get_flag(EmitterFlag::IsFuncSimple) && self.scopes.len() != 1;
        let needs_special_updating = hints.get_flag(EmitterFlag::HandleUnarySpecials);

        let name_info = if !name_in_locator && !name_in_access && !name_lacks_env {
            let Some(var_key_info) = self.resolve_global_string(lexeme, true) else { eprintln!("Could not resolve env-var name '{lexeme}' here."); return hints.without_flag(EmitterFlag::IsVisitOK); };

            self.emit_unary_inst(Opcode::PushStr, var_key_info.id, 0);

            let env_access_ip = self.emit_nonary_inst(Opcode::GetVar, 0);
            self.put_ic_of_inst(env_access_ip);

            (
                None,
                Opcode::Ret,
                false, // Continue-generation flag: If off, exit this function early.
            )
        } else if !name_in_locator && !name_in_access && name_lacks_env {  // ! add missing case for "in_locator" for simple, stack-offset local names...
            self.capturing_names.insert(lexeme.to_owned());
            (
                self.resolve_local(lexeme).or_else(|| {
                    self.resolve_global(lexeme)
                }),
                Opcode::GetLocal,
                !needs_special_updating,
            )
        } else if !name_in_locator && name_in_access && !name_lacks_env {
            (
                self.resolve_global_string(lexeme, true).or_else(|| {
                    self.record_global_string(lexeme, true)
                }),
                Opcode::PushStr,
                !needs_special_updating,
            )
        } else if name_in_locator && !name_in_access && !name_lacks_env {
            // ? Skip emission of LHS for assignment exprs to prevent a messy stack situation... Just put its environment object key.
            (
                self.resolve_global_string(lexeme, true).or_else(|| {
                    self.record_global_string(lexeme, true)
                }),
                Opcode::PushStr,
                !needs_special_updating,
            )
        } else {
            eprintln!("\n\tNote: unhandled type of named-expr at ~ line#{}", self.line);
            (
                None,
                Opcode::Ret,
                false,
            )
        };
        
        self.cached_info = name_info.0;

        if name_info.2 {
            let name_opcode = name_info.1;
            let Some(named_info) = self.cached_info else { return hints.without_flag(EmitterFlag::IsVisitOK); };
            
            self.emit_unary_inst(name_opcode, named_info.id, 0);
        }

        hints
    }

    fn emit_nil_node(&mut self, _: &str, _: &SyntaxData, _: &AST, hints: EmitterHints) -> EmitterHints {
        self.emit_nonary_inst(Opcode::PushUndef, 0);
        hints
    }

    fn emit_literal(&mut self, source: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Literal(lt_tk_pos) = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };
        let Token {begin, end, line, kind} = ast.tokens[*lt_tk_pos];
        let literal_lexeme = &source[begin as usize .. end as usize];

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            if kind == TokenKind::Identifier && self.resolve_local(literal_lexeme).is_none() {
                // detect captured name
                hints.disable_flag(EmitterFlag::IsFuncSimple);
            }

            return hints;
        } else if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        self.line = line;

        match kind {
            TokenKind::LiteralUndefined => {
                self.emit_nonary_inst(Opcode::PushUndef, 0);
                hints
            },
            TokenKind::LiteralNull => {
                self.emit_nonary_inst(Opcode::PushNull, 0);
                hints
            },
            TokenKind::LiteralNaN => {
                self.emit_nonary_inst(Opcode::PushNaN, 0);
                hints
            },
            TokenKind::LiteralInfinity => {
                self.emit_nonary_inst(Opcode::PushInf, 0);
                hints
            },
            TokenKind::LiteralTrue | TokenKind::LiteralFalse => {
                self.emit_nonary_inst(Opcode::PushBool, if kind == TokenKind::LiteralTrue {1} else {0});
                hints
            },
            TokenKind::LiteralDecInt => {
                if let Some(num_cid) = self.record_constant(literal_lexeme, JSValue::Number(str::parse::<i32>(literal_lexeme).expect("Unexpected malformed decimal int at emitter.rs.") as f64)) {
                    self.emit_unary_inst(Opcode::PushConst, num_cid.id, 0);
                    hints
                } else {
                    hints.without_flag(EmitterFlag::IsVisitOK)
                }
            },
            TokenKind::LiteralOctInt => {
                if let Some(num_cid) = self.record_constant(literal_lexeme, JSValue::Number(i32::from_str_radix(literal_lexeme, 8).expect("Unexpected malformed octal int at emitter.rs.") as f64)) {
                    self.emit_unary_inst(Opcode::PushConst, num_cid.id, 0);
                    hints
                } else {
                    hints.without_flag(EmitterFlag::IsVisitOK)
                }
            },
            TokenKind::LiteralBinInt => {
                if let Some(num_cid) = self.record_constant(literal_lexeme, JSValue::Number(i32::from_str_radix(literal_lexeme, 2).expect("Unexpected malformed binary int at emitter.rs.") as f64)) {
                    self.emit_unary_inst(Opcode::PushConst, num_cid.id, 0);
                    hints
                } else {
                    hints.without_flag(EmitterFlag::IsVisitOK)
                }
            },
            TokenKind::LiteralHexInt => {
                if let Some(num_cid) = self.record_constant(literal_lexeme, JSValue::Number(i32::from_str_radix(literal_lexeme, 16).expect("Unexpected malformed hexadecimal int at emitter.rs.") as f64)) {
                    self.emit_unary_inst(Opcode::PushConst, num_cid.id, 0);
                    hints
                } else {
                    hints.without_flag(EmitterFlag::IsVisitOK)
                }
            },
            TokenKind::LiteralFloat => {
                if let Some(num_cid) = self.record_constant(literal_lexeme, JSValue::Number(str::parse::<f64>(literal_lexeme).unwrap_or(f64::NAN))) {
                    self.emit_unary_inst(Opcode::PushConst, num_cid.id, 0);
                    hints
                } else {
                    hints.without_flag(EmitterFlag::IsVisitOK)
                }
            },
            TokenKind::LiteralString => {
                if let Some(gs_info) = self.record_global_string(
                    literal_lexeme, false
                ) {
                    if let Some(gsc_info) = self.record_constant(literal_lexeme, JSValue::StringId(gs_info.id)) {
                        self.emit_unary_inst(Opcode::PushConst, gsc_info.id, 0);
                        hints
                    } else {
                        hints.without_flag(EmitterFlag::IsVisitOK)
                    }
                } else {
                    hints.without_flag(EmitterFlag::IsVisitOK)
                }
            },
            TokenKind::KeywordThis => {
                self.emit_nonary_inst(Opcode::PushThisRef, 0);
                hints
            },
            TokenKind::Identifier => self.emit_name(literal_lexeme, hints),
            _ => {
                hints.without_flag(EmitterFlag::IsVisitOK)
            }
        }
    }

    #[allow(unused)]
    fn emit_object_expr(&mut self, source: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        if hints.get_flag(EmitterFlag::CheckFnIsSimple) || hints.get_flag(EmitterFlag::PrepassVars) {
            // ! FIXME: simplicity check should check each item for foreign names.
            return hints;
        }

        hints.without_flag(EmitterFlag::IsVisitOK) // todo: implement AFTER object literal support in parser.
    }

    fn emit_array_expr(&mut self, source: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        if hints.get_flag(EmitterFlag::CheckFnIsSimple) || hints.get_flag(EmitterFlag::PrepassVars) {
            // ! FIXME: simplicity check should check each item for foreign names.
            return hints;
        }

        self.emit_nonary_inst(Opcode::MakeObj, 0);

        let Some(array_proto_info) = self.resolve_global(JS_ARRAY_PROTO_ALIAS) else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        self.emit_unary_inst(Opcode::SetProto, array_proto_info.id, 1);

        let SyntaxData::ArrayExpr {items} = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        for (item_pos, item) in items.iter().enumerate() {
            if !self.emit_node(source, item.as_ref(), ast, hints).check_ok() {
                hints.without_flag(EmitterFlag::IsVisitOK);
            }

            let set_arr_item_ip = self.emit_unary_inst(Opcode::SetOwnProp, item_pos as i32, 0);
            self.put_ic_of_inst(set_arr_item_ip);
        }

        hints
    }

    #[allow(unused)]
    fn emit_lambda(&mut self, source: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        // todo: implement AFTER object support!
        hints.without_flag(EmitterFlag::IsVisitOK)
    }

    fn emit_lhs(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Lhs {accesses, source} = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            if let SyntaxData::Literal(name_token_id) = &source.as_ref().data {
                let source_expect_msg = format!("Expected name token in LHS at tokens position {}", *name_token_id);
                let source_token = ast.tokens.get(*name_token_id).expect(&source_expect_msg);
                let source_name_lexeme = source_token.to_str(src_text);

                if self.resolve_local(source_name_lexeme).is_none() {
                    hints.disable_flag(EmitterFlag::IsFuncSimple);
                }
            }

            return hints;
        }

        if !self.emit_node(src_text, source, ast, hints).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        for (access_pos, (access_direct, access_expr)) in accesses.iter().enumerate() {            
            if !self.emit_node(src_text, access_expr, ast, hints.without_flag(EmitterFlag::InLocator)).check_ok() {
                eprintln!("\n\tNote: check LHS {0} access #{access_pos} around line {1}.", if *access_direct {"[key]"} else {"'.'"}, self.line);
                return hints.without_flag(EmitterFlag::IsVisitOK);
            }

            // ? Leave the last emitted key for LHS accesses, as there's SET_PROP VM opcodes that require that temporary value!
            if !hints.get_flag(EmitterFlag::InLocator) {
                let get_prop_ip = self.emit_nonary_inst(Opcode::GetProp, 0);

                self.put_ic_of_inst(get_prop_ip);
            } else {
                break;
            }
        }

        hints.with_flag(EmitterFlag::InAccess)
    }

    fn emit_unary(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Unary { inner, op, prefix } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            let _ = self.emit_node(src_text, inner, ast, hints);

            return hints;
        }

        #[allow(unused)]
        let (mut temp_opcode, op_is_prefix) = match op {
            Operator::NegBool => (
                Some(Opcode::ForceBool),
                true
            ),
            Operator::NegNum => (
                Some(Opcode::NegNum),
                true
            ),
            Operator::ForceNum => (
                Some(Opcode::ForceNum),
                true
            ),
            Operator::Inc => (
                match inner.data.get_emitter_id() {
                    SyntaxId::Literal => Some(if hints.get_flag(EmitterFlag::IsFuncSimple) {
                        Opcode::IncLocal
                    } else {
                        Opcode::IncProp
                    }),
                    SyntaxId::Lhs => Some(Opcode::IncProp),
                    _ => None
                },
                *prefix
            ),
            Operator::Dec => (
                match inner.data.get_emitter_id() {
                    SyntaxId::Literal => Some(if hints.get_flag(EmitterFlag::IsFuncSimple) {
                        Opcode::DecLocal
                    } else {
                        Opcode::DecProp
                    }),
                    SyntaxId::Lhs => Some(Opcode::DecProp),
                    _ => None
                },
                *prefix
            ),
            Operator::Delete => (
                // if matches!(inner.data.get_emitter_id(), SyntaxId::Literal | SyntaxId::Lhs) {
                //     Some(Opcode::DelProp)
                // } else { None }, // todo
                None,
                true
            ),
            Operator::TypeOf => (None, true), // todo
            Operator::Void => (Some(Opcode::Discard), true),
            _ => (None, true)
        };

        if temp_opcode.is_none() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let handle_new_unary = *op == Operator::New;
        let handle_special_unary = matches!(*op, Operator::Inc | Operator::Dec);

        if !self.emit_node(src_text, inner, ast,
            if handle_new_unary {
                hints.with_flag(EmitterFlag::InConstruction)
            } else if handle_special_unary {
                hints.with_flag(EmitterFlag::HandleUnarySpecials).with_flag(EmitterFlag::InLocator)
            } else { hints }
        ).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let Some(temp_opcode_v) = temp_opcode else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if handle_new_unary {
            // ! ctor calls are handled by emit_call()
            return hints;
        } else if handle_special_unary && self.cached_info.is_some() {
            let SymbolInfo { id, tag } = self.cached_info.unwrap();

            if self.scopes.len() == 1 {
                // ! In top-level code, the global env is globalThis -- Account for that as the target object here...
                self.emit_nonary_inst(Opcode::PushThisRef, 0);
            }

            let temp_op_arg = match tag {
                SymbolTag::Local => id,
                SymbolTag::KeyStr => id,
                _ => 0,
            };

            // ! prefix ++ or -- of any expr are handled specially since old/new values are discarded!
            let unary_op_ip = self.emit_unary_inst(
                temp_opcode_v,
                temp_op_arg,
                if *prefix {1 << 15} else {0}
            ); // ? prefix mode is an opcode flag, as adding more opcodes for this small case would be overkill.

            self.put_ic_of_inst(unary_op_ip);

            return hints;
        }

        self.emit_nonary_inst(temp_opcode_v, if *prefix {1} else {0});

        hints
    }

    #[allow(unused)]
    fn emit_binary(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        let SyntaxData::Binary { l, r, op } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            let _ = self.emit_node(src_text, l, ast, hints);
            let _ = self.emit_node(src_text, r, ast, hints);

            return hints;
        }

        // TODO: implement logical && , || generation.
        // if !self.emit_logical_juncts(src_text, l, r, ast) { return hints.without_flag(EmitterFlag::IsVisitOK); }

        struct DirOp {
            pub is_ltr: bool,
            pub opcode: Opcode,
        };
        type MaybeDirOp = Option<DirOp>;

        // ? Here, match operators carefully & note their evaluation direction in JS. Once a pair of these values is determined, use that info if present to emit an op after LHS and RHS are emitted for the stack operands.
        let eval_dir_and_opcode = match *op {
            Operator::ModuloNum => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::Mod
            }),
            Operator::Mul => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::Mul
            }),
            Operator::Div => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::Div
            }),
            Operator::Add => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::Add
            }),
            Operator::Sub => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::Sub
            }),
            Operator::BitAnd => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::BtAnd
            }),
            Operator::BitOr => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::BtOr
            }),
            Operator::StrictEqual => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::StrictEq
            }),
            Operator::StrictUnequal => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::StrictNe
            }),
            Operator::LooseEqual => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::LooseEq
            }),
            Operator::LooseUnequal => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::LooseNe
            }),
            _ => None
        };

        if eval_dir_and_opcode.is_none() {
            eprintln!("Invalid operator in binary-expr.");
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let DirOp { is_ltr, opcode } = eval_dir_and_opcode.expect("Expected DirOp value at emitter.rs ~ line#622.");

        if is_ltr {
            if !self.emit_node(src_text, l, ast, hints).check_ok() {
                return hints.without_flag(EmitterFlag::IsVisitOK);
            }

            if !self.emit_node(src_text, r, ast, hints).check_ok() {
                return hints.without_flag(EmitterFlag::IsVisitOK);
            }
        } else {
            if !self.emit_node(src_text, r, ast, hints).check_ok() {
                return hints.without_flag(EmitterFlag::IsVisitOK);
            }

            if !self.emit_node(src_text, l, ast, hints).check_ok() {
                return hints.without_flag(EmitterFlag::IsVisitOK);
            }
        }

        self.emit_nonary_inst(opcode, 0);

        hints
    }

    fn emit_assign(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Assign {dest, src} = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            let _ = self.emit_node(src_text, src, ast, hints);

            return hints;
        }

        let lhs_hints = self.emit_node(src_text, dest, ast, hints.with_flag(EmitterFlag::InLocator));

        if !lhs_hints.check_ok() {
            eprintln!("\n\tNote: Invalid destination of assign-expr.");
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }
   
        if !self.emit_node(src_text, src, ast, hints.without_flag(EmitterFlag::InLocator)).check_ok() {
            eprintln!("\n\tNote: Invalid source of assign-expr.");
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        if lhs_hints.get_flag(EmitterFlag::InAccess) {
            let set_prop_ic_ip = self.emit_nonary_inst(Opcode::SetProp, 0);

            self.put_ic_of_inst(set_prop_ic_ip);
            // ! FIXME: if this emitter fn is called in an emit_function_decl() AND any names need an env:
            // ! ...rely on passed flags to have IsFuncSimple = OFF.
        } else if let Some(saved_local_info) = self.cached_info && hints.get_flag(EmitterFlag::IsFuncSimple) {
            self.emit_unary_inst(Opcode::SetLocal, saved_local_info.id, 0);
        } else {
            let set_var_ip = self.emit_nonary_inst(Opcode::SetVar, 0);

            self.put_ic_of_inst(set_var_ip);
        }

        hints
    }

    fn emit_call(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Call { args, callee } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            let _ = self.emit_node(src_text, callee, ast, hints);

            return hints;
        } 

        // ! FIXME: when methods are supported, use Dup2 after these.
        if !self.emit_node(src_text, callee, ast, hints).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let arg_count = args.len() as i32;

        for (arg_pos, arg_expr) in args.iter().enumerate() {
            if !self.emit_node(src_text, arg_expr, ast, hints).check_ok() {
                eprintln!("\n\tNote: see argument-expr #{arg_pos} in call ~ line {}", self.line);
                return hints.without_flag(EmitterFlag::IsVisitOK);
            }
        }

        if hints.get_flag(EmitterFlag::InConstruction) {
            self.emit_unary_inst(Opcode::CallCtor, arg_count, 0);
        } else {
            self.emit_unary_inst(Opcode::Call, arg_count, 0);
        }

        hints
    }

    #[allow(unused)]
    fn emit_func_decl(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        if hints.get_flag(EmitterFlag::IsFuncSimple) {
            return hints;
        }

        // ? Steps:
        // ? 1. Skip if prepass.
        // ? 2. Push scope and map param names!
        // ? 3. Generate body: simplicity check, prepass, generate! Flags, if the name is in the set of capturing functions' names, will have simple-func flag = OFF.
        // ? 4. Put chunk into preloaded function object in heap.
        // ? 5. Exit scope!

        hints.without_flag(EmitterFlag::IsVisitOK)
    }

    fn emit_block(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Block {stmts} = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }


        for (stmt_pos, stmt_node) in stmts.iter().enumerate() {
            if !self.emit_node(src_text, stmt_node, ast, hints.with_flag(EmitterFlag::PrepassVars)).check_ok() {
                eprintln!("\n\tNote (block prepass): See statement #{stmt_pos} in block ~ line {}", self.line);
                return hints.without_flag(EmitterFlag::IsVisitOK);
            }
        }

        for (stmt_pos_2, stmt_node_2) in stmts.iter().enumerate() {
            if !self.emit_node(src_text, stmt_node_2, ast, hints.without_flag(EmitterFlag::PrepassVars)).check_ok() {
                eprintln!("\n\tNote (block emission): See statement #{stmt_pos_2} in block ~ line {}", self.line);
                return hints.without_flag(EmitterFlag::IsVisitOK);
            }
        }

        hints
    }

    fn emit_vars(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Vars { vars } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::PrepassVars) {
            for (var_name_tk_pos, _) in vars.iter() {
                let name_lexeme = ast.tokens[*var_name_tk_pos].to_str(src_text);

                if hints.get_flag(EmitterFlag::IsFuncSimple) {
                    let _ = self.record_local(name_lexeme);
                } else {
                    let _ = self.record_global_string(name_lexeme, true);
                }

                self.local_reserve_n += 1;
            }

            return hints;
        }

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            for (_, var_init_expr_2) in vars.iter() {
                let _ = self.emit_node(src_text, var_init_expr_2.as_ref().expect("Expected variable node in emitter.rs ~ line#880.").as_ref(), ast, hints);
            }

            return hints;
        }

        if hints.get_flag(EmitterFlag::IsFuncSimple) {
            for (var_name_tk_pos_2, var_init_expr_2) in vars.iter() {
                if !self.emit_node(src_text, var_init_expr_2.as_ref().expect("Expected variable node in emitter.rs ~ line#888.").as_ref(), ast, hints.without_flag(EmitterFlag::InLocator)).check_ok() {
                    return hints.without_flag(EmitterFlag::IsVisitOK);
                }

                let local_name = ast.tokens[*var_name_tk_pos_2].to_str(src_text);
                let local_expect_msg = format!("Expected local of '{}' in emitter.rs ~ line#797.", local_name);

                let local_id = self.resolve_local(local_name).expect(&local_expect_msg);
                self.emit_unary_inst(Opcode::SetLocal, local_id.id, 0);
            }
        } else {
            for (var_name_tk_pos_2, var_init_expr_2) in vars.iter() {
                let local_key_name = ast.tokens[*var_name_tk_pos_2].to_str(src_text);

                let local_str_id = self.resolve_global_string(local_key_name, true).or_else(|| {
                    self.record_global_string(local_key_name, true)
                }).expect("Expected local name info created at emitter.rs ~ line#904.");

                self.emit_unary_inst(Opcode::PushStr, local_str_id.id, 0);

                if let Some(var_initializer) = var_init_expr_2 {
                    if !self.emit_node(src_text, var_initializer, ast, hints.without_flag(EmitterFlag::InLocator)).check_ok() {
                        eprintln!("Error: variable name {local_key_name} lacks a valid initializer.");
                        return hints.without_flag(EmitterFlag::IsVisitOK);
                    }
                } else {
                    self.emit_nonary_inst(Opcode::PushUndef, 0);
                }

                let var_init_ic_ip = self.emit_nonary_inst(Opcode::SetVar, 0);
                self.put_ic_of_inst(var_init_ic_ip);
            }
        }

        hints
    }

    #[allow(unused)]
    fn emit_ifs(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Ifs {cond, t_block, f_block} = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::PrepassVars) || hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            let _ = self.emit_node(src_text, t_block, ast, hints);
            let _ = self.emit_node(src_text, f_block, ast, hints);

            return hints;
        }

        if !self.emit_node(src_text, cond, ast, hints).check_ok() {
            eprintln!("if-stmt condition expr is invalid!");
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let skip_t_block_pos = self.emit_unary_inst(Opcode::JumpElse, 0, 1);

        if !self.emit_node(src_text, t_block, ast, hints).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        if f_block.is_empty_stmt() {
            let early_end_pos = self.chunks.last().unwrap().code.len() as i32;

            self.chunks.last_mut().expect("Expected available chunk for emitting to in non-else if-stmt; emitter.rs ~ line#851.").code[skip_t_block_pos as usize].arg = early_end_pos - skip_t_block_pos;
            return hints;
        }

        let skip_f_block_pos = self.emit_unary_inst(Opcode::Jump, 0, 0);
        self.chunks.last_mut().expect("Expected available chunk for emitting to in else clause; emitter.rs ~ line#955.").code[skip_t_block_pos as usize].arg = skip_f_block_pos + 1 - skip_t_block_pos;

        if !self.emit_node(src_text, f_block, ast, hints).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let end_if_else_pos = self.chunks.last().expect("Expected available chunk at emitter.rs ~ line#961.").code.len() as i32;
        self.chunks.last_mut().expect("Expected available chunk for emitting to after if-else-stmt; emitter.rs ~ line#962.").code[skip_f_block_pos as usize].arg = end_if_else_pos - skip_f_block_pos;

        hints
    }

    #[allow(unused)]
    fn emit_while(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let SyntaxData::While { cond, body } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::PrepassVars) || hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            let _ = self.emit_node(src_text, body, ast, hints);
            return hints;
        }

        let loop_check_ip = self.chunks.last().expect("Expected available chunk at emitter.rs ~ line#971.").code.len() as i32;
        if !self.emit_node(src_text, cond, ast, hints).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let loop_exit_jump_ip = self.chunks.last().expect("Expected available chunk at emitter.rs ~ line#976.").code.len() as i32;
        self.emit_unary_inst(Opcode::JumpElse, 0, 0);

        if !self.emit_node(src_text, body, ast, hints).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let loop_repeat_jump_ip = self.chunks.last().expect("Expected available chunk at emitter.rs ~ line#983.").code.len() as i32;
        self.emit_unary_inst(Opcode::Jump, loop_check_ip - loop_repeat_jump_ip, 0);

        let loop_end_ip = loop_repeat_jump_ip + 1;
        self.emit_unary_inst(Opcode::PopN, 0, 1);
        self.chunks.last_mut().expect("Expected available chunk for emitting to after if-else-stmt; emitter.rs ~ line#987.").code[loop_exit_jump_ip as usize].arg = loop_end_ip - loop_exit_jump_ip;

        hints
    }

    #[allow(unused)]
    fn emit_c_like_for(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        hints.without_flag(EmitterFlag::IsVisitOK)
    }

    #[allow(unused)]
    fn emit_break(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        hints.without_flag(EmitterFlag::IsVisitOK) // todo: implement LATER!
    }

    #[allow(unused)]
    fn emit_continue(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        hints.without_flag(EmitterFlag::IsVisitOK) // todo: implement LATER!
    }

    fn emit_return(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Return { out } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::PrepassVars) || hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            return hints;
        }

        if !self.emit_node(src_text, out, ast, hints.without_flag(EmitterFlag::InLocator)).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        self.emit_nonary_inst(Opcode::Ret, 0);

        hints
    }

    fn emit_expr_stmt(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let SyntaxData::ExprStmt { inner } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::PrepassVars) || hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            return hints;
        }

        self.emit_node(src_text, inner, ast, hints.without_flag(EmitterFlag::InLocator))
    }

    fn emit_empty_stmt(&mut self, _: &str, _: &SyntaxData, _: &AST, hints: EmitterHints) -> EmitterHints {
        hints
    }

    fn emit_node(&mut self, src_text: &str, node: &SyntaxNode, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let node_data = &node.data;

        match node_data {
            SyntaxData::Nil => self.emit_nil_node(src_text, node_data, ast, hints),
            SyntaxData::Literal(_) => self.emit_literal(src_text, node_data, ast, hints),
            // SyntaxData::ObjectExpr { .. } => self.emit_object_expr(src_text, node_data, ast, hints),
            SyntaxData::ArrayExpr { .. } => self.emit_array_expr(src_text, node_data, ast, hints),
            // SyntaxData::Lambda { .. } => self.emit_object_expr(src_text, node_data, ast, hints),
            SyntaxData::Lhs { .. } => self.emit_lhs(src_text, node_data, ast, hints),
            SyntaxData::Unary { .. } => self.emit_unary(src_text, node_data, ast, hints),
            SyntaxData::Binary { .. } => self.emit_binary(src_text, node_data, ast, hints),
            SyntaxData::Assign { .. } => self.emit_assign(src_text, node_data, ast, hints),
            SyntaxData::Call { .. } => self.emit_call(src_text, node_data, ast, hints),
            SyntaxData::FuncDecl { .. } => self.emit_func_decl(src_text, node_data, ast, hints),
            SyntaxData::Block { .. } => self.emit_block(src_text, node_data, ast, hints),
            SyntaxData::Vars { .. } => self.emit_vars(src_text, node_data, ast, hints),
            SyntaxData::Ifs { .. } => self.emit_ifs(src_text, node_data, ast, hints),
            SyntaxData::While { .. } => self.emit_while(src_text, node_data, ast, hints),
            // SyntaxData::CLikeFor { .. } => self.emit_c_like_for(src_text, node_data, ast, hints),
            // SyntaxData::Break { .. } => self.emit_break(src_text, node_data, ast, hints),
            // SyntaxData::Continue { .. } => self.emit_continue(src_text, node_data, ast, hints),
            SyntaxData::Return { .. } => self.emit_return(src_text, node_data, ast, hints),
            SyntaxData::ExprStmt { .. } => self.emit_expr_stmt(src_text, node_data, ast, hints),
            SyntaxData::EmptyStmt { .. } => self.emit_empty_stmt(src_text, node_data, ast, hints),
            _ => {
                eprintln!("Syntax at line {} is unsupported.", self.line);
                hints.without_flag(EmitterFlag::IsVisitOK)
            },
        }
    }

    pub fn emit_code(&mut self, ast: &AST) -> Option<Program> {
        let AST {txt, decls, name, ..} = ast;

        let initial_hints = EmitterHints::default().with_flag(EmitterFlag::IsVisitOK);

        for (decl_pos, decl_stmt) in decls.iter().enumerate() {
            if !self.emit_node(txt.as_str(), decl_stmt, ast, initial_hints.with_flag(EmitterFlag::PrepassVars)).check_ok() {
                eprintln!("\n\tNote (check pass): See invalid declaration #{decl_pos}.");
                return None;
            }
        }

        for (decl_pos, decl_stmt) in decls.iter().enumerate() {
            if !self.emit_node(txt.as_str(), decl_stmt, ast, initial_hints).check_ok() {
                eprintln!("\n\tNote (emit pass): See invalid declaration #{decl_pos} at line {}.", self.line);
                return None;
            }
        }

        self.emit_nonary_inst(Opcode::Ret, 0);

        Some(Program {
            heap: std::mem::take(&mut self.heap),
            spool: std::mem::take(&mut self.spool),
            top_level: std::mem::take(self.chunks.first_mut().expect("Expected top-level bytecode chunk present at Emitter::emit_code ~ line 970.")),
            name: name.clone(),
        })
    }
}
