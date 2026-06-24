use std::fmt::Display;
use std::cell::RefCell;

#[allow(unused_imports)]
use crate::{
    backend::emitter::{Emitter, SymbolInfo, SymbolTag}, frontend::{
        token::TokenKind, ast::AST, lexer::Lexer, parser::Parser
    }, runtime::{
        code::{JSGlobalConstID, Program, dump_bytecode}, ctx::{EvalStatus, JSContext, NativeFn}, objects::{ExoticObject}, opaque::JSOpaque, property::{Property}, shape::Shape, values::JSValue, vm::run_vm
    }
};

pub const RUNTIME_NAME: &str = "    ______                    \n   / ____/__  ______________  \n  / /_  / _ \\/ ___/ ___/ __ \\ \n / __/ /  __/ /  / /  / /_/ / \n/_/    \\___/_/  /_/   \\____/ \n";

const FERRO_HEAP_POP: usize = 4096;
const FERRO_STRINGS_POP: usize = 4096;
const FERRO_SHAPES_POP: usize = 4096;
const FERRO_STACK_MAX: usize = 16384;
const FERRO_RECUR_MAX: usize = 80;
const FERRO_VERSION_MAJOR: u16 = 0;
const FERRO_VERSION_MINOR: u16 = 2;
const FERRO_VERSION_PATCH: u16 = 0;

pub struct DriverConfig {
    pub name: &'static str,
    pub author: &'static str,
    pub heap_limit: usize,
    pub strings_limit: usize,
    pub shape_limit: usize,
    pub stack_limit: usize,
    pub recursion_limit: usize,
    pub v_major: u16,
    pub v_minor: u16,
    pub v_patch: u16,
}

impl Default for DriverConfig {
    fn default() -> Self {
        Self {
            name: RUNTIME_NAME,
            author: "DrkWithT",
            heap_limit: FERRO_HEAP_POP,
            strings_limit: FERRO_STRINGS_POP,
            shape_limit: FERRO_SHAPES_POP,
            stack_limit: FERRO_STACK_MAX,
            recursion_limit: FERRO_RECUR_MAX,
            v_major: FERRO_VERSION_MAJOR,
            v_minor: FERRO_VERSION_MINOR,
            v_patch: FERRO_VERSION_PATCH
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DriverFlag {
    ShowVersion = (1 << 0),
    ShowHelp = (1 << 1),
    DumpBytecode = (1 << 2),
    RunScript = (1 << 3),
}

pub struct Driver {
    pub emitter: Emitter,
    pub config: DriverConfig,
    pub lexicals: Vec<(&'static str, TokenKind)>,
    pub passed_script_path: String,
}

impl Display for Driver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let DriverConfig { name, author, v_major, v_minor, v_patch, .. } = self.config;

        write!(f, "\x1b[1;31m{name}\x1b[0m\n\n\x1b[0m\n\x1b[1;30mv{v_major}.{v_minor}.{v_patch}\x1b[0m --- {author}\n")
    }
}

impl Driver {
    pub fn new(config: DriverConfig) -> Self {
        Self {
            emitter: Emitter::new(config.shape_limit, config.heap_limit, config.strings_limit),
            config,
            lexicals: vec![],
            passed_script_path: String::default()
        }
    }

    pub fn add_lexical(&mut self, s: &'static str, tag: TokenKind) {
        self.lexicals.push((s, tag));
    }

    pub fn add_global_string(&mut self, id: JSGlobalConstID, s: &'static str) {
        self.emitter.set_global_constant_of_str(id, s);
    }

    pub fn add_global_object(&mut self, symbol: &'static str, in_proto: JSValue, props: Vec<(String, JSValue, u8)>, opaque: JSOpaque) -> JSValue {
        let global_oid = self.emitter.heap.add_item(Some(RefCell::new(
            ExoticObject {
                props: vec![],
                items: vec![],
                in_proto,
                out_proto: JSValue::Undefined,
                opaque,
                shape: 0 // ! This is the ID of the 1st shape stored in `emitter.shapes: ShapePool`, specifically for blank objects.
            }
        )));

        if global_oid.is_none() {
            return JSValue::Null;
        }

        let global_shape_id = self.emitter.shapes.store(Shape::default());

        if global_shape_id.is_none() {
            return JSValue::Null;
        }

        let global_oid = global_oid.unwrap();
        let mut global_shape_id = global_shape_id.unwrap();

        for (prop_name, prop_value, prop_flags) in props.iter() {
            let prop_name_sid = self.emitter.spool.add_item(Some(Box::new(prop_name.clone())));

            if prop_name_sid.is_none() {
                eprintln!("Failed to create property of name '{}' for built-in object.", prop_name.as_str());
                return JSValue::Null
            }

            self.emitter.heap.get_item_mut(global_oid).as_mut().unwrap().props.push(
                Property::data(prop_value, *prop_flags)
            );

            let next_global_shape_id = self.emitter.shapes.next_sid;

            let temp_shape = self.emitter.shapes.fetch_mut(global_shape_id).unwrap().derive_child(prop_name_sid.unwrap() as usize, next_global_shape_id);

            if let Some(new_shape_id) = self.emitter.shapes.store(temp_shape) {
                global_shape_id = new_shape_id;
            } else {
                eprintln!("Failed to create new shape for built-in object.");
                return JSValue::Null;
            }
        }

        self.emitter.scopes.first_mut().unwrap().symbols.insert(symbol.to_owned(), SymbolInfo {
            id: global_oid,
            tag: SymbolTag::GlobalObj
        });

        JSValue::ObjectId(global_oid)
    }

    fn compile_script(&mut self, source: &str) -> Option<Program> {
        let mut tokenizer = Lexer::new(source);

        for (special_text, special_tag) in self.lexicals.iter() {
            tokenizer.map_special_lexical(special_text, *special_tag);
        }

        let tokens = tokenizer.lex_all(source);
        let mut parser = Parser::new(&tokens, source);
        let decls = parser.parse_data()?;

        let ast = AST {
            txt: source.to_owned(),
            tokens,
            decls,
            name: self.passed_script_path.clone()
        };

        self.emitter.emit_code(&ast)
    }

    /// ### ABOUT
    /// Runs the interpreter given cmdline bitflags and a view of source text. Returns the VM's resulting value and the VM status.
    pub fn run(&mut self, source: &str, flags: u8) -> (JSValue, EvalStatus) {
        if 0 != (flags & DriverFlag::ShowVersion as u8) {
            println!("{self}");
            return (JSValue::Undefined, EvalStatus::Ok);
        }

        if 0 != (flags & DriverFlag::ShowHelp as u8) {
            println!("usage: ferrojs [-v | -h | -d, -r <JS file>]");
            return (JSValue::Undefined, EvalStatus::Ok);
        }

        let program = self.compile_script(source);

        if program.is_none() {
            return (JSValue::Undefined, EvalStatus::CompileError);
        }

        let program = program.unwrap();

        if 0 != (flags & DriverFlag::DumpBytecode as u8) {
            dump_bytecode(&program);
        }

        if 0 == (flags & DriverFlag::RunScript as u8) {
            return (JSValue::Undefined, EvalStatus::Ok);
        }

        let mut state = JSContext::new(self.config.heap_limit, self.config.stack_limit, self.config.recursion_limit as u16, program);

        run_vm(&mut state)
    }
}
