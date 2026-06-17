use std::collections::{HashMap};
use std::cell::{RefCell};

use crate::backend::emitter::EmitterFlag::InLocator;
use crate::frontend::{
    token::{TokenKind, Token},
    ast::{Operator, SyntaxId, SyntaxData, SyntaxNode, PropDecl, PropDeclTag, AST},
};

use crate::runtime::opaque::JSOpaque;
use crate::runtime::objects::{ExoticObject};
#[allow(unused)]
use crate::runtime::{
    code::{InlineCache, JSFuncFlag, Opcode, JS_DELETE_FLAG_NOOP, JS_DELETE_FLAG_LOOSE, JS_DELETE_FLAG_STRICT, Instruction, JSGlobalConstID, JS_GLOBAL_CONST_N, Chunk, Program},
    values::{JSValue},
    objects::{JSObjPtr, JSStrPtr, JS_OBJECT_COST, JS_STRING_COST, ItemPool, /*ExoticObject*/},
    // funcs::{FuncBody, JSFunction},
};


pub const JS_BOOLEAN_PROTO_ALIAS: &str = "[[Boolean.prototype]]";
pub const JS_NUMBER_PROTO_ALIAS: &str = "[[Number.prototype]]";
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
    /// For temporarily "declaring" params before a function needs-env check.
    Placeholder,
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
    /// Does a member access for a callee need its `thisArg` filled?
    HasThisArg = (1 << 8),
    InMethod = (1 << 9),
    /// Has the sub-AST compilation succeeded?
    IsVisitOK = (1 << 10),
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
    pub chunk_buf: Vec<Box<Chunk>>,
    pub chunks: Vec<Chunk>,
    pub scopes: Vec<SymbolScope>,
    pub gconsts: Vec<JSValue>,
    pub cached_info: Option<SymbolInfo>,
    pub local_reserve_n: i32,
    pub line: u16,
    pub chunk_n: u16,
}

impl Emitter {
    pub fn new(object_population: usize, str_population: usize) -> Self {
        Self {
            heap: ItemPool::<JSObjPtr, JS_OBJECT_COST>::new(object_population),
            spool: ItemPool::<JSStrPtr, JS_STRING_COST>::new(str_population),
            chunk_buf: vec![],
            chunks: vec![Chunk::default()],
            scopes: vec![SymbolScope::default()],
            gconsts: {
                let mut global_consts = Vec::<JSValue>::with_capacity(JS_GLOBAL_CONST_N);

                global_consts.resize(JS_GLOBAL_CONST_N, JSValue::Undefined);
                global_consts
            },
            cached_info: None,
            local_reserve_n: 0,
            line: 1,
            chunk_n: 0
        }
    }

    pub fn set_global_constant_of_str(&mut self, id: JSGlobalConstID, s: &'static str) -> bool {
        if let Some(sid) = self.record_global_string(s, false) {   
            self.gconsts[id as usize] = JSValue::StringId(sid.id);
            return true;
        }

        false
    }

    // pub fn set_global_constant_of_obj(&mut self, id: usize, symbol: &'static str, o: JSObjPtr) -> bool { false } // todo: use for setting up JS intrinics...

    fn put_ic_of_inst(&mut self, inst_pos: i32) {
        let next_ic_id = self.chunks.last().expect("Expected available chunk at emitter.rs ~ put_ic_of_inst.").icaches.len() as u16;

        self.chunks.last_mut().unwrap().icaches.push(InlineCache::default());
        self.chunks.last_mut().unwrap().code[inst_pos as usize].flags |= next_ic_id;
    }

    #[allow(unused)]
    fn put_dead_ic_of_inst(&mut self, inst_pos: i32) {
        self.chunks.last_mut().unwrap().icaches.push(InlineCache::dead());
        self.chunks.last_mut().unwrap().code[inst_pos as usize].flags |= u16::MAX;
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
        if s.is_empty() {
            return None;
        }
        
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

        let real_string_symbol = if is_key { format!("[[{s}]]") } else { format!("'{}'", s.to_owned()) };
        let real_string = s.to_owned();
        let real_string: JSStrPtr = Some(Box::new(real_string));

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

    fn record_escaped_string(&mut self, tk: Token, is_key: bool, source: &str) -> Option<SymbolInfo> {
        let lexeme = tk.to_str(source);

        if let Some(pre_info) = self.resolve_global_string(lexeme, is_key) {
            return Some(pre_info);
        }

        let real_string_symbol = if is_key { format!("[[{lexeme}]]") } else { lexeme.to_owned() };
        let real_string_cell: JSStrPtr = Some(Box::new(
            tk.to_unescaped_string(source)
        ));

        if let Some(sid) = self.spool.add_item(real_string_cell) {   
            let temp_info = SymbolInfo {
                id: sid,
                tag: if is_key {SymbolTag::KeyStr} else {SymbolTag::GlobalStr},
            };

            self.scopes.first_mut().expect("Expected global scope to be tracked in Emitter::record_escaped_string()!").symbols.insert(real_string_symbol, temp_info);
            
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

            if !s.is_empty() {
                current_scope.symbols.insert(s.to_owned(), temp_info);
            }

            current_scope.next_const_id += 1;
            self.chunks.last_mut().expect("Expected available chunk for Emitter::record_constant()!").consts.push(v);

            Some(temp_info)
        }
    }

    fn emit_name(&mut self, lexeme: &str, hints: EmitterHints) -> EmitterHints {
        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            if !self.scopes.last().unwrap().symbols.contains_key(lexeme) {
                // println!("Debug emitter.rs: '{lexeme}' -> possible capture");
                return hints.without_flag(EmitterFlag::IsFuncSimple);
            } else {
                // println!("Debug emitter.rs: '{lexeme}' -> local");
                return hints;
            }
        }

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        let name_in_locator = hints.get_flag(EmitterFlag::InLocator);
        let name_in_access = hints.get_flag(EmitterFlag::InAccess);
        let name_lacks_env = hints.get_flag(EmitterFlag::IsFuncSimple) && self.scopes.len() == 2;
        let needs_special_updating = hints.get_flag(EmitterFlag::HandleUnarySpecials);

        let name_info = if !name_in_locator && !name_in_access && !name_lacks_env {
            // ! FIXME: Test this logic- record (with implicit resolve check) a global key string.
            let Some(var_key_info) = self.record_global_string(lexeme, true) else { eprintln!("Could not resolve env-var name '{lexeme}' here."); return hints.without_flag(EmitterFlag::IsVisitOK); };

            self.emit_unary_inst(Opcode::PushStr, var_key_info.id, 0);

            let env_access_ip = self.emit_nonary_inst(Opcode::GetVar, 0);
            self.put_ic_of_inst(env_access_ip);

            (
                None,
                Opcode::Ret,
                false, // Continue-generation flag: If off, exit this function early.
            )
        } else if !name_in_locator && !name_in_access && name_lacks_env {
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
        } else if name_in_access || !name_lacks_env {
            // println!("Debug emitter.rs: '{lexeme}' -> possible object key...");
            // ? Skip emission of LHS for assignment exprs to prevent a messy stack situation... Just put its environment object key.
            (
                self.resolve_global_string(lexeme, true).or_else(|| {
                    self.record_global_string(lexeme, true)
                }),
                Opcode::PushStr,
                !needs_special_updating,
            )
        } else {
            (
                None,
                Opcode::Ret,
                false,
            )
        };
        
        self.cached_info = name_info.0;

        if name_info.2 {
            let name_opcode = name_info.1;
            let Some(named_info) = self.cached_info else { println!("\n\tNote: could not resolve name '{lexeme}'."); return hints.without_flag(EmitterFlag::IsVisitOK); };
            
            self.emit_unary_inst(name_opcode, named_info.id, 0);
        }

        hints
    }

    fn emit_nil_node(&mut self, _: &str, _: &SyntaxData, _: &AST, hints: EmitterHints) -> EmitterHints {
        if hints.get_flag(EmitterFlag::CheckFnIsSimple) || hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        self.emit_nonary_inst(Opcode::PushUndef, 0);        
        hints
    }

    fn emit_literal(&mut self, source: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Literal(lt_tk_pos) = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };
        let Token {begin, end, line, kind} = ast.tokens[*lt_tk_pos];
        let literal_lexeme = &source[begin as usize .. end as usize];

        self.line = line;

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            if kind == TokenKind::Identifier {
                return self.emit_name(literal_lexeme, hints);
            }

            return hints;
        }

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

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
                if let Some(num_cid) = self.record_constant(literal_lexeme, JSValue::Number(i32::from_str_radix(&literal_lexeme[2..], 2).expect("Unexpected malformed binary int at emitter.rs.") as f64)) {
                    self.emit_unary_inst(Opcode::PushConst, num_cid.id, 0);
                    hints
                } else {
                    hints.without_flag(EmitterFlag::IsVisitOK)
                }
            },
            TokenKind::LiteralHexInt => {
                if let Some(num_cid) = self.record_constant(literal_lexeme, JSValue::Number(i32::from_str_radix(&literal_lexeme[2..], 16).expect("Unexpected malformed hexadecimal int at emitter.rs.") as f64)) {
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
            TokenKind::LiteralEscapedString => {
                if let Some(gs_info) = self.record_escaped_string(
                    ast.tokens[*lt_tk_pos].clone(), false, source
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

    fn emit_object_expr(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::ObjectExpr { props } = node else {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            for prop_decl in props {
                if !self.emit_node(src_text, prop_decl.initializer.as_ref(), ast, hints).get_flag(EmitterFlag::IsFuncSimple) {
                    hints.disable_flag(EmitterFlag::IsFuncSimple);
                }
            }

            return hints;
        }

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        // ! FIXME: Add support for intrinsic IDs of included prototypes, etc. used here. An example would be Array's ctor ref and `Array.prototype`!
        self.emit_nonary_inst(Opcode::MakeObj, 0);

        for (prop_pos, PropDecl { initializer, name_tk_id, tag }) in props.iter().enumerate() {
            let temp_key_tk = ast.tokens.get(*name_tk_id).unwrap();
            let temp_key_str = temp_key_tk.to_str(src_text);

            let Some(key_sid) = self.record_global_string(temp_key_str, true) else { return hints.without_flag(EmitterFlag::IsVisitOK); };

            self.emit_unary_inst(Opcode::PushStr, key_sid.id, 0);

            if !self.emit_node(src_text, initializer, ast, hints.without_flag(EmitterFlag::InLocator).with_flag(EmitterFlag::InMethod)).check_ok() {
                eprintln!("\n\tNote: Invalid property decl #{} found in object at source:{}", prop_pos, self.line);
                return hints.without_flag(EmitterFlag::IsVisitOK);
            }

            let set_prop_ip = match *tag {
                PropDeclTag::Data => {
                    self.emit_unary_inst(Opcode::SetProp, 1, 0)
                },
                PropDeclTag::Getter => {
                    self.emit_unary_inst(Opcode::SetProp, 2, 0)
                },
                PropDeclTag::Setter => {
                    self.emit_unary_inst(Opcode::SetProp, 3, 0)
                }
            };

            self.put_ic_of_inst(set_prop_ip);
        }

        hints
    }

    // TODO: test later, arrays not implemented for now.
    fn emit_array_expr(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::ArrayExpr {items} = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            for arr_item in items.iter() {
                if !self.emit_node(src_text, arr_item, ast, hints).get_flag(EmitterFlag::IsFuncSimple) {
                    hints.disable_flag(EmitterFlag::IsFuncSimple);
                }
            }

            return hints;
        }

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        self.emit_nonary_inst(Opcode::MakeObj, 0);

        let Some(array_proto_info) = self.resolve_global(JS_ARRAY_PROTO_ALIAS) else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        self.emit_unary_inst(Opcode::SetProto, array_proto_info.id, 1);

        #[allow(unused)]
        for (item_pos, item) in items.iter().enumerate() {
            if !self.emit_node(src_text, item.as_ref(), ast, hints).check_ok() {
                hints.without_flag(EmitterFlag::IsVisitOK);
            }

            // ! FIXME: implement after note is followed.
            todo!("Implement arrays after object mechanics for accessors work!!");
            // TODO: Call Array ctor here when intrinsic mapping is implemented.
        }

        hints
    }

    fn emit_lambda(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Lambda { params, body } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        self.scopes.push(SymbolScope {
            symbols: HashMap::<String, SymbolInfo>::default(),
            next_local_id: 1,
            next_const_id: 0,
            next_ic_id: 0
        });

        // ! Pre-record params as local names to avoid false capture-fn positives. These temp name-info entries will be removed after the capture check pass.
        let mut pre_params = Vec::<String>::default();

        for pre_param_tk_pos in params.iter() {
            let pre_param_name = ast.tokens[*pre_param_tk_pos].to_string(src_text);
            pre_params.push(pre_param_name.clone());
            self.scopes.last_mut().unwrap().symbols.insert(pre_param_name, SymbolInfo { id: 0, tag: SymbolTag::Placeholder }); // ! Fake "declare" params to know of them during the capture checking pass.
        }

        let pre_param_check_hints = (if self.scopes.len() == 2 {
            hints.with_flag(EmitterFlag::IsFuncSimple)
        } else {
            hints.without_flag(EmitterFlag::IsFuncSimple)
        }).with_flag(EmitterFlag::CheckFnIsSimple);

        // ? Pre-check during generation via prepass: if there's nested scopes / captured names / closure returns, an environment is needed at runtime for the function (now considered complex).
        let func_is_simple = self.emit_node(src_text, body, ast, pre_param_check_hints).get_flag(EmitterFlag::IsFuncSimple) || hints.get_flag(EmitterFlag::InMethod);

        // ! Delete the mock parameter entries to not confuse the emission's name resolution.
        self.scopes.last_mut().unwrap().symbols.clear();

        let started_chunk = Chunk {
            arity: params.len() as u16,
            icaches: Vec::with_capacity(4),
            ..Default::default()
        };
        self.chunks.push(started_chunk);

        for (param_pos, param_tk_pos) in params.iter().enumerate() {
            let param_name = ast.tokens[*param_tk_pos].to_string(src_text);

            if func_is_simple {
                let _ = self.record_local(param_name.as_str());
            } else {
                // ? NOTE: For complex lambdas, copies of real argument values are stored in the environment as needed, leaving the old argument values preserved for the Arguments object later.
                let param_key_sid = self.record_global_string(param_name.as_str(), true).expect("Expected valid env-key info for param at emitter.rs ~ emit_lambda at params.");

                self.emit_unary_inst(Opcode::PushStr, param_key_sid.id, 0);
                self.emit_unary_inst(Opcode::GetLocal, param_pos as i32 + 1, 0);
                self.emit_nonary_inst(Opcode::InitVar, 0);
            }
        }

        let func_specific_hints = if func_is_simple {
            hints.enable_flag(EmitterFlag::IsFuncSimple);
            hints.with_flag(EmitterFlag::IsFuncSimple)
        } else {
            hints.disable_flag(EmitterFlag::IsFuncSimple);
            hints.without_flag(EmitterFlag::IsFuncSimple)
        };

        // println!("func_specific_hints.is_func_simple for lambda body emission... {}", func_specific_hints.get_flag(EmitterFlag::IsFuncSimple));

        if !self.emit_node(
            src_text,
            body,
            ast,
            func_specific_hints
                .without_flag(EmitterFlag::CheckFnIsSimple)
                .without_flag(EmitterFlag::PrepassVars)
        ).check_ok() {
            self.scopes.pop();

            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let code_ptr = std::ptr::from_mut(self.chunk_buf.push_mut(
            Box::new(self.chunks.pop().unwrap())
        ).as_mut());

        // ! Mark function code as requiring an environment via compile-time heuristics, as this will be checked at runtime to decide whether to not elide an environment allocation / use local offsets instead.
        unsafe {
            code_ptr.as_mut_unchecked().flags = if !func_is_simple {JSFuncFlag::NeedsEnv as u8} else {0};
        }

        let Some(func_oid) = self.heap.add_item(Some(RefCell::new(
            ExoticObject::with_opaque(JSOpaque::bytecode(code_ptr))
        ))) else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        self.scopes.pop();

        let Some(func_const_id) = self.record_constant("", JSValue::ObjectId(func_oid)) else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        self.emit_unary_inst(Opcode::PushConst, func_const_id.id, 0);

        // ! Do not create closures at scope-level 1, as returned functions from top-level code are dead closures anyways.
        if !func_specific_hints.get_flag(EmitterFlag::IsFuncSimple) && self.scopes.len() > 1 {
            self.emit_nonary_inst(Opcode::MakeClosure, 0);
        }

        hints
    }

    fn emit_lhs(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Lhs {accesses, source} = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            if let SyntaxData::Literal(name_token_id) = &source.as_ref().data {
                let source_expect_msg = format!("Expected name token in LHS at tokens position {}", *name_token_id);
                let source_token = ast.tokens.get(*name_token_id).expect(&source_expect_msg);
                let source_name_lexeme = source_token.to_str(src_text);

                if self.resolve_local(source_name_lexeme).is_none() {
                    hints.disable_flag(EmitterFlag::IsFuncSimple);
                }

                for (bracketed_access, access_expr) in accesses.iter() {
                    if *bracketed_access && !self.emit_node(src_text, access_expr, ast, hints).get_flag(EmitterFlag::IsFuncSimple) {
                        hints.disable_flag(EmitterFlag::IsFuncSimple);
                    }
                }
            }

            return hints;
        }

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        if !self.emit_node(src_text, source, ast, hints.without_flag(EmitterFlag::InLocator)).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        if hints.get_flag(EmitterFlag::HasThisArg) {
            self.emit_nonary_inst(Opcode::Dup1, 0); // This is determined by the outer visitation ONLY IF this one is a LHS within a Call.
        }

        for (access_pos, (access_direct, access_expr)) in accesses.iter().enumerate() {            
            if !self.emit_node(src_text, access_expr, ast, hints.without_flag(EmitterFlag::InLocator).with_flag(EmitterFlag::InAccess)).check_ok() {
                eprintln!("\n\tNote: check LHS {0} access #{access_pos} around line {1}.", if *access_direct {"[key]"} else {"'.'"}, self.line);
                return hints.without_flag(EmitterFlag::IsVisitOK);
            }

            // ? Leave the last emitted key for LHS accesses, as there's SET_PROP VM opcodes that require that temporary value!
            if !hints.get_flag(EmitterFlag::InLocator) || access_pos != accesses.len() - 1 {
                let get_prop_ip = self.emit_unary_inst(Opcode::GetProp, 1, 0);

                self.put_ic_of_inst(get_prop_ip);
            } else {
                break;
            }
        }

        hints.with_flag(EmitterFlag::InAccess)
    }

    fn emit_deletion(&mut self, src_text: &str, node: &SyntaxNode, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let inner_expr_flags = self.emit_node(src_text, node, ast, hints);

        if !inner_expr_flags.check_ok() {
            return inner_expr_flags;
        }

        let opcode_flags: u16 = if inner_expr_flags.get_flag(EmitterFlag::InAccess) {
            JS_DELETE_FLAG_LOOSE
        } else {
            JS_DELETE_FLAG_NOOP
        };

        self.emit_unary_inst(Opcode::Delete, 0, opcode_flags);

        hints.without_flag(InLocator)
    }

    fn emit_unary(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Unary { inner, op, prefix } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            if !self.emit_node(src_text, inner, ast, hints).get_flag(EmitterFlag::IsFuncSimple) {
                hints.disable_flag(EmitterFlag::IsFuncSimple);
            }

            return hints;
        }

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        let (temp_opcode, _op_is_prefix) = match op {
            Operator::NegBool => (
                Some(Opcode::NegBool),
                true
            ),
            Operator::ForceNum => (
                Some(Opcode::ForceNum),
                true
            ),
            Operator::NegNum => (
                Some(Opcode::NegNum),
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
            Operator::BitFlip => (Some(Opcode::BtFlip), true),
            Operator::Delete => (
                Some(Opcode::Delete),
                true
            ),
            Operator::TypeOf => (
                Some(Opcode::GetType),
                true
            ),
            Operator::Void => (Some(Opcode::Discard), true),
            _ => (None, true)
        };

        if temp_opcode.is_none() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        if *op == Operator::Delete {
            return self.emit_deletion(src_text, inner, ast, hints.with_flag(EmitterFlag::InLocator));
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

    fn emit_binary_logical(&mut self, src_text: &str, op: Operator, l: &SyntaxNode, r: &SyntaxNode, ast: &AST, hints: EmitterHints) -> EmitterHints {
        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        let jumper_opcode = match op {
            Operator::LogicalAnd => Some(Opcode::JumpElse),
            Operator::LogicalOr => Some(Opcode::JumpIf),
            _ => None
        };

        if jumper_opcode.is_none() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let jumper_opcode = jumper_opcode.unwrap();

        if !self.emit_node(src_text, l, ast, hints.without_flag(EmitterFlag::InLocator)).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let rhs_skip_ip = self.emit_unary_inst(jumper_opcode, 0, 0);

        if !self.emit_node(src_text, r, ast, hints.without_flag(EmitterFlag::InLocator)).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let end_rhs_skip_ip = self.chunks.last().unwrap().code.len() as i32;
        self.chunks.last_mut().unwrap().code.get_mut(rhs_skip_ip as usize).unwrap().arg = end_rhs_skip_ip - rhs_skip_ip;

        hints
    }

    #[allow(unused)]
    fn emit_binary(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        let SyntaxData::Binary { l, r, op } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            let lhs_no_need_env = self.emit_node(src_text, l, ast, hints).get_flag(EmitterFlag::IsFuncSimple);
            let rhs_no_need_env = self.emit_node(src_text, r, ast, hints).get_flag(EmitterFlag::IsFuncSimple);

            if !lhs_no_need_env || !rhs_no_need_env {
                hints.disable_flag(EmitterFlag::IsFuncSimple);
            }

            return hints;
        }

        if matches!(*op, Operator::LogicalAnd | Operator::LogicalOr) {
            return self.emit_binary_logical(src_text, *op, l, r, ast, hints);
        }

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
            Operator::BitLShift => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::BtLs
            }),
            Operator::BitRShift => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::BtRs
            }),
            Operator::BitAnd => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::BtAnd
            }),
            Operator::BitXor => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::BtXor
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
            Operator::Lesser => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::Lt
            }),
            Operator::Greater => Some(DirOp {
                is_ltr: true,
                opcode: Opcode::Gt
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

    fn emit_cond(&mut self, src_text: &str, node_data: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Cond {check, l, r} = node_data else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            if !self.emit_node(src_text, check, ast, hints).get_flag(EmitterFlag::IsFuncSimple) {
                hints.disable_flag(EmitterFlag::IsFuncSimple);
            }

            if !self.emit_node(src_text, l, ast, hints).get_flag(EmitterFlag::IsFuncSimple) {
                hints.disable_flag(EmitterFlag::IsFuncSimple);
            }

            if !self.emit_node(src_text, r, ast, hints).get_flag(EmitterFlag::IsFuncSimple) {
                hints.disable_flag(EmitterFlag::IsFuncSimple);
            }

            return hints;
        }

        if !self.emit_node(src_text, check, ast, hints.without_flag(InLocator)).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let cond_jump_ip = self.chunks.last().expect("Expected available chunk at emitter.rs ~ emit_cond for cond LHS.").code.len() as i32;
        self.emit_unary_inst(Opcode::JumpElse, 0, 0);

        if !self.emit_node(src_text, l, ast, hints.without_flag(EmitterFlag::InLocator)).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let cond_skip_r_ip = self.chunks.last().expect("Expected available chunk at emitter.rs ~ emit_cond for cond RHS.").code.len() as i32;
        self.emit_unary_inst(Opcode::Jump, 0, 0);

        let cond_begin_r_ip = cond_skip_r_ip + 1;
        if !self.emit_node(src_text, r, ast, hints.without_flag(EmitterFlag::InLocator)).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let cond_end_ip = self.chunks.last().unwrap().code.len() as i32;

        self.chunks.last_mut().unwrap().code.get_mut(cond_jump_ip as usize).unwrap().arg = cond_begin_r_ip - cond_jump_ip;
        self.chunks.last_mut().unwrap().code.get_mut(cond_skip_r_ip as usize).unwrap().arg = cond_end_ip - cond_skip_r_ip;

        hints
    }

    fn emit_assign(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Assign {dest, src} = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            if !self.emit_node(src_text, src, ast, hints).get_flag(EmitterFlag::IsFuncSimple) {
                hints.disable_flag(EmitterFlag::IsFuncSimple);
            }

            return hints;
        }

        if hints.get_flag(EmitterFlag::PrepassVars) {
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
        } else if let Some(saved_local_info) = self.cached_info && hints.get_flag(EmitterFlag::IsFuncSimple) {
            self.emit_unary_inst(Opcode::SetLocal, saved_local_info.id, 0);
        } else {
            let set_var_ip = self.emit_nonary_inst(Opcode::SetVar, 0);

            self.put_ic_of_inst(set_var_ip);
        }

        hints
    }

    fn emit_call(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Call { args, callee } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            if !self.emit_node(src_text, callee, ast, hints).get_flag(EmitterFlag::IsFuncSimple) || (callee.data.get_emitter_id() == SyntaxId::Lambda && self.scopes.len() > 1) {
                hints.disable_flag(EmitterFlag::IsFuncSimple);
            }

            for arg_expr in args.iter() {
                if !self.emit_node(src_text, arg_expr, ast, hints).get_flag(EmitterFlag::IsFuncSimple) {
                    hints.disable_flag(EmitterFlag::IsFuncSimple);
                }
            }

            return hints;
        }

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        // ? Evaluate thisArg for callees.
        let callee_emit_hints = if callee.data.get_emitter_id() == SyntaxId::Lhs {
            hints.with_flag(EmitterFlag::HasThisArg)
        } else if self.scopes.len() < 2 {
            self.emit_nonary_inst(Opcode::PushThisRef, 0); // Fill in globalThis = [[global-env]].

            hints
        } else {
            self.emit_nonary_inst(Opcode::PushUndef, 0);

            hints
        };

        if !self.emit_node(src_text, callee, ast, callee_emit_hints).check_ok() {
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

    fn emit_function_decl(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let SyntaxData::FuncDecl { params, body, name_tk_id } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            if self.scopes.len() > 2 {
                return hints.without_flag(EmitterFlag::IsFuncSimple);
            }

            return hints;
        }

        let func_name = ast.tokens[*name_tk_id].to_str(src_text);
        let outer_code_is_simple = hints.get_flag(EmitterFlag::IsFuncSimple);

        // ? Treat a function decl like a hoisted `var name = <function>`.
        if hints.get_flag(EmitterFlag::PrepassVars) {
            if !outer_code_is_simple {
                let _ = self.record_global_string(func_name, true);
            } else {
                let _ = self.record_local(func_name);
            }

            return hints;
        }

        let callee_name_info = SymbolInfo {
            id: 0,
            tag: SymbolTag::Local
        };

        self.scopes.push(SymbolScope {
            symbols: {
                let mut temp_symbols = HashMap::<String, SymbolInfo>::default();

                temp_symbols.insert(func_name.to_owned(), callee_name_info); // ! NOTE: callee is always at LOCAL 0 AKA CALLEE_BP[0] on the stack!

                temp_symbols
            },
            next_local_id: 1,
            next_const_id: 0,
            next_ic_id: 0
        });

        // ! Pre-record params as local names to avoid false capture-fn positives. These temp name-info entries will be removed after the capture check pass.
        let mut pre_params = Vec::<String>::default();

        for pre_param_tk_pos in params.iter() {
            let pre_param_name = ast.tokens[*pre_param_tk_pos].to_string(src_text);
            pre_params.push(pre_param_name.clone());
            self.scopes.last_mut().unwrap().symbols.insert(pre_param_name, SymbolInfo { id: 0, tag: SymbolTag::Placeholder }); // ! Fake "declare" params to know of them during the capture checking pass.
        }

        let pre_param_check_hints = (if self.scopes.len() == 2 {
            hints.with_flag(EmitterFlag::IsFuncSimple)
        } else {
            hints.without_flag(EmitterFlag::IsFuncSimple)
        }).with_flag(EmitterFlag::CheckFnIsSimple);

        // ? Pre-check during generation via prepass: if there's nested scopes / captured names / closure returns, an environment is needed at runtime for the function (now considered complex).
        let func_is_simple = self.emit_node(src_text, body, ast, pre_param_check_hints).get_flag(EmitterFlag::IsFuncSimple);

        // ! Delete the mock symbol entries to not confuse the emission's name resolution.
        self.scopes.last_mut().unwrap().symbols.clear();
        self.scopes.last_mut().unwrap().symbols.insert(func_name.to_owned(), callee_name_info);

        let func_specific_hints = if func_is_simple {
            hints.with_flag(EmitterFlag::IsFuncSimple)
        } else {
            hints.without_flag(EmitterFlag::IsFuncSimple)
        };

        self.chunks.push(Chunk {
            arity: params.len() as u16,
            icaches: Vec::with_capacity(4),
            ..Default::default()
        });

        for (param_pos, param_tk_pos) in params.iter().enumerate() {
            let param_name = ast.tokens[*param_tk_pos].to_string(src_text);

            if func_is_simple {
                let _ = self.record_local(param_name.as_str());
            } else {
                // ? NOTE: For complex functions, copies of real argument values are stored in the environment as needed, leaving the old argument values preserved for the Arguments object later.
                let param_key_sid = self.record_global_string(param_name.as_str(), true).expect("Expected valid env-key info for param at emitter.rs ~ emit_function_decl at params.");

                self.emit_unary_inst(Opcode::PushStr, param_key_sid.id, 0);
                self.emit_unary_inst(Opcode::GetLocal, param_pos as i32 + 1, 0);
                self.emit_nonary_inst(Opcode::InitVar, 0);
            }
        }

        if !self.emit_node(src_text, body, ast, func_specific_hints).check_ok() {
            self.scopes.pop();
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        let code_ptr = std::ptr::from_mut(self.chunk_buf.push_mut(
            Box::new(self.chunks.pop().unwrap())
        ).as_mut());

        // ! Mark function code as requiring an environment via compile-time heuristics, as this will be checked at runtime to decide whether to not elide an environment allocation / use local offsets instead.
        unsafe {
            code_ptr.as_mut_unchecked().flags = if !func_is_simple {JSFuncFlag::NeedsEnv as u8} else {0};
        }

        let Some(func_oid) = self.heap.add_item(Some(RefCell::new(
            ExoticObject::with_opaque(
                JSOpaque::bytecode(code_ptr)
            )
        ))) else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        self.scopes.pop();

        let Some(func_const_id) = self.record_constant(func_name, JSValue::ObjectId(func_oid)) else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        // ? NOTE: Check if enclosing code requires an environment, thus needing the function to be dynamically bound to a name.
        if !outer_code_is_simple {
            if let Some(outer_func_key_locus) = self.resolve_global_string(func_name, true) {
                self.emit_unary_inst(Opcode::PushStr, outer_func_key_locus.id, 0);
                self.emit_unary_inst(Opcode::PushConst, func_const_id.id, 0);

                // ! Like emit_lambda, do not allocate any closure wrapping a top-level function, as it would be unreachable after program finish.
                if !func_specific_hints.get_flag(EmitterFlag::IsFuncSimple) && self.scopes.len() > 1 {
                    self.emit_nonary_inst(Opcode::MakeClosure, 0);
                }

                let set_func_prop_ip = self.emit_nonary_inst(Opcode::InitVar, 0);
                self.put_ic_of_inst(set_func_prop_ip);

                hints
            } else {
                hints.without_flag(EmitterFlag::IsVisitOK)
            }
        } else if let Some(func_local_locus) = self.resolve_local(func_name) {
            // ? NOTE: For non-env functions, the lambda constant is stored in a local slot.
            self.emit_unary_inst(Opcode::PushConst, func_const_id.id, 0);
            self.emit_unary_inst(Opcode::SetLocal, func_local_locus.id, 0);

            hints
        } else {
            hints.without_flag(EmitterFlag::IsVisitOK)
        }
    }

    fn emit_block(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Block {stmts} = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            let mut not_all_stmts_simple = false;

            for simplicity_check_stmt in stmts.iter() {
                if matches!(simplicity_check_stmt.data.get_emitter_id(), SyntaxId::FuncDecl | SyntaxId::Lambda) {
                    not_all_stmts_simple = true;
                    break;
                    // ! Below, mark prepass flag under simplicity check since we could be prepassing a lambda with nested lambdas. Lambda emission doesn't prematurely stop visitation upon simple-checks but prepass-vars ON.
                } else if !self.emit_node(src_text, simplicity_check_stmt, ast, hints.with_flag(EmitterFlag::PrepassVars)).get_flag(EmitterFlag::IsFuncSimple) {
                    not_all_stmts_simple = true;
                    break;
                }
            }

            if not_all_stmts_simple {
                hints.disable_flag(EmitterFlag::IsFuncSimple);
            }

            return hints;
        }

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        for (stmt_pos, stmt_node) in stmts.iter().enumerate() {
            let nested_stmt_hints = self.emit_node(src_text, stmt_node, ast, hints.with_flag(EmitterFlag::PrepassVars));

            if !nested_stmt_hints.check_ok() {
                eprintln!("\n\tNote (block prepass): See statement #{stmt_pos} in block ~ line {}", self.line);
                return hints.without_flag(EmitterFlag::IsVisitOK);
            } else if !nested_stmt_hints.get_flag(EmitterFlag::IsFuncSimple) {
                hints.disable_flag(EmitterFlag::IsFuncSimple);
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

    fn emit_vars(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Vars { vars } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            for (var_name_tk_pos, var_init_expr_2) in vars.iter() {
                // ! Record var name as dud info if checking for captures, etc.
                let pre_var_name = ast.tokens.get(*var_name_tk_pos).unwrap().to_string(src_text);
                self.scopes.last_mut().unwrap().symbols.insert(pre_var_name, SymbolInfo { id: 0, tag: SymbolTag::Placeholder });

                if !self.emit_node(src_text, var_init_expr_2.as_ref().expect("Expected variable node in emitter.rs ~ line#880.").as_ref(), ast, hints).get_flag(EmitterFlag::IsFuncSimple) {
                    hints.disable_flag(EmitterFlag::IsFuncSimple);
                }
            }

            return hints;
        }

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

                let var_init_ic_ip = self.emit_nonary_inst(Opcode::InitVar, 0);
                self.put_ic_of_inst(var_init_ic_ip);
            }
        }

        hints
    }

    #[allow(unused)]
    fn emit_ifs(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::Ifs {cond, t_block, f_block} = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            let tblock_no_need_env = self.emit_node(src_text, t_block, ast, hints).get_flag(EmitterFlag::IsFuncSimple);
            let fblock_no_need_env = self.emit_node(src_text, f_block, ast, hints).get_flag(EmitterFlag::IsFuncSimple);

            if !tblock_no_need_env || !fblock_no_need_env {
                hints.disable_flag(EmitterFlag::IsFuncSimple);
            }

            return hints;
        }

        if hints.get_flag(EmitterFlag::PrepassVars) {
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

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) || hints.get_flag(EmitterFlag::PrepassVars) {
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

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            // ? NOTE: Conservatively assume that returning any `function() {}` expr is a closure, thus requiring _an environment_ captured by it.
            return if let SyntaxData::Lambda { .. } = out.data {
                hints.without_flag(EmitterFlag::IsFuncSimple)
            } else {
                self.emit_node(src_text, out, ast, hints)
            };
        }

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        if !self.emit_node(src_text, out, ast, hints.without_flag(EmitterFlag::InLocator)).check_ok() {
            return hints.without_flag(EmitterFlag::IsVisitOK);
        }

        self.emit_nonary_inst(Opcode::Ret, 0);

        hints
    }

    fn emit_expr_stmt(&mut self, src_text: &str, node: &SyntaxData, ast: &AST, mut hints: EmitterHints) -> EmitterHints {
        let SyntaxData::ExprStmt { inner } = node else { return hints.without_flag(EmitterFlag::IsVisitOK); };

        if hints.get_flag(EmitterFlag::CheckFnIsSimple) {
            if !self.emit_node(src_text, inner, ast, hints).get_flag(EmitterFlag::IsFuncSimple) {
                hints.disable_flag(EmitterFlag::IsFuncSimple);
            }

            return hints;
        }

        if hints.get_flag(EmitterFlag::PrepassVars) {
            return hints;
        }

        let result_hints = self.emit_node(src_text, inner, ast, hints.without_flag(EmitterFlag::InLocator));

        // Discard temporary after expression's effects.
        self.emit_unary_inst(Opcode::PopN, 0, 1);
        result_hints
    }

    fn emit_empty_stmt(&mut self, _: &str, _: &SyntaxData, _: &AST, hints: EmitterHints) -> EmitterHints {
        hints
    }

    fn emit_node(&mut self, src_text: &str, node: &SyntaxNode, ast: &AST, hints: EmitterHints) -> EmitterHints {
        let node_data = &node.data;

        match node_data {
            SyntaxData::Nil => self.emit_nil_node(src_text, node_data, ast, hints),
            SyntaxData::Literal(_) => self.emit_literal(src_text, node_data, ast, hints),
            SyntaxData::ObjectExpr {..} => self.emit_object_expr(src_text, node_data, ast, hints),
            SyntaxData::ArrayExpr {..} => self.emit_array_expr(src_text, node_data, ast, hints),
            SyntaxData::Lambda {..} => self.emit_lambda(src_text, node_data, ast, hints),
            SyntaxData::Lhs {..} => self.emit_lhs(src_text, node_data, ast, hints),
            SyntaxData::Unary {..} => self.emit_unary(src_text, node_data, ast, hints),
            SyntaxData::Binary {..} => self.emit_binary(src_text, node_data, ast, hints),
            SyntaxData::Cond {..} => self.emit_cond(src_text, node_data, ast, hints),
            SyntaxData::Assign {..} => self.emit_assign(src_text, node_data, ast, hints),
            SyntaxData::Call {..} => self.emit_call(src_text, node_data, ast, hints),
            SyntaxData::FuncDecl {..} => self.emit_function_decl(src_text, node_data, ast, hints),
            SyntaxData::Block {..} => self.emit_block(src_text, node_data, ast, hints),
            SyntaxData::Vars {..} => self.emit_vars(src_text, node_data, ast, hints),
            SyntaxData::Ifs {..} => self.emit_ifs(src_text, node_data, ast, hints),
            SyntaxData::While {..} => self.emit_while(src_text, node_data, ast, hints),
            // SyntaxData::CLikeFor {..} => self.emit_c_like_for(src_text, node_data, ast, hints),
            // SyntaxData::Break {..} => self.emit_break(src_text, node_data, ast, hints),
            // SyntaxData::Continue {..} => self.emit_continue(src_text, node_data, ast, hints),
            SyntaxData::Return {..} => self.emit_return(src_text, node_data, ast, hints),
            SyntaxData::ExprStmt {..} => self.emit_expr_stmt(src_text, node_data, ast, hints),
            SyntaxData::EmptyStmt {..} => self.emit_empty_stmt(src_text, node_data, ast, hints),
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

        self.chunk_buf.push(Box::new(
            self.chunks.pop().unwrap()
        )); // ! Finish top-level code last.

        Some(Program {
            heap: std::mem::take(&mut self.heap),
            spool: std::mem::take(&mut self.spool),
            chunks: std::mem::take(&mut self.chunk_buf),
            global_consts: std::mem::take(&mut self.gconsts),
            name: name.clone(),
        })
    }
}
