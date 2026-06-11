pub mod frontend;
pub mod backend;
pub mod runtime;

use std::{env::args, fs::read_to_string, process::ExitCode};

use crate::{backend::emitter::Emitter, frontend::{ast::AST, lexer::Lexer, parser::Parser, token::TokenKind}, runtime::{code::dump_bytecode, ctx::{JSContext, EvalStatus}, objects::DEFAULT_SHAPE_POPULATION, vm::{DEFAULT_JS_RECUR_LIMIT, DEFAULT_JS_STACK_SIZE, run_vm}}};


const RUNTIME_NAME: &str = "    ______                    \n   / ____/__  ______________  \n  / /_  / _ \\/ ___/ ___/ __ \\ \n / __/ /  __/ /  / /  / /_/ / \n/_/    \\___/_/  /_/   \\____/ \n";

fn main() -> ExitCode {
    let mut file_name: Option<String> = None;
    let mut option_show_version = false;
    let mut option_show_help = false;
    let mut option_allow_bc_dump = false;

    for arg in args().skip(1) {
        match arg.as_str() {
            "-v" => { option_show_version = true; },
            "-h" => { option_show_help = true; },
            "-d" => { option_allow_bc_dump = true; },
            _ => file_name = Some(arg)
        }
    }

    if option_show_version {
        println!("\x1b[1;31m{RUNTIME_NAME}\x1b[0m\n\x1b[1;30mv0.0.1\x1b[0m --- By: DrkWithT (GitHub)\n");
        return ExitCode::SUCCESS;
    } else if option_show_help {
        println!("usage: ferrojs [-v | -h | [-d | -r] <JS file>]\n");
        return ExitCode::SUCCESS;
    }

    let source_fpath = String::from("main.js");
    let source_fpath = file_name.as_ref().unwrap_or(&source_fpath).as_str();
    let source_txt = read_to_string(source_fpath);

    if source_txt.is_err() {
        eprintln!("File at {source_fpath} could not be read.");
        return ExitCode::FAILURE;
    }

    let source_txt = source_txt.unwrap();

    let mut tokenizer = Lexer::new(&source_txt);
    tokenizer.map_special_lexical("var", TokenKind::KeywordVar);
    tokenizer.map_special_lexical("if", TokenKind::KeywordIf);
    tokenizer.map_special_lexical("else", TokenKind::KeywordElse);
    tokenizer.map_special_lexical("while", TokenKind::KeywordWhile);
    tokenizer.map_special_lexical("for", TokenKind::KeywordFor);
    tokenizer.map_special_lexical("return", TokenKind::KeywordReturn);
    tokenizer.map_special_lexical("function", TokenKind::KeywordFunction);
    tokenizer.map_special_lexical("new", TokenKind::KeywordNew);
    tokenizer.map_special_lexical("this", TokenKind::KeywordThis);
    tokenizer.map_special_lexical("typeof", TokenKind::KeywordTypeOf);
    tokenizer.map_special_lexical("void", TokenKind::KeywordVoid);
    tokenizer.map_special_lexical("delete", TokenKind::KeywordDelete);
    tokenizer.map_special_lexical("get", TokenKind::KeywordGet);
    tokenizer.map_special_lexical("set", TokenKind::KeywordSet);
    tokenizer.map_special_lexical("++", TokenKind::OperatorPlusPlus);
    tokenizer.map_special_lexical("--", TokenKind::OperatorMinusMinus);
    tokenizer.map_special_lexical("!", TokenKind::OperatorBang);
    tokenizer.map_special_lexical("%", TokenKind::OperatorPct);
    tokenizer.map_special_lexical("*", TokenKind::OperatorTimes);
    tokenizer.map_special_lexical("/", TokenKind::OperatorSlash);
    tokenizer.map_special_lexical("+", TokenKind::OperatorPlus);
    tokenizer.map_special_lexical("-", TokenKind::OperatorMinus);
    tokenizer.map_special_lexical("===", TokenKind::OperatorStrictEquals);
    tokenizer.map_special_lexical("!==", TokenKind::OperatorStrictUnequals);
    tokenizer.map_special_lexical("==", TokenKind::OperatorLooseEqual);
    tokenizer.map_special_lexical("!=", TokenKind::OperatorLooseUnequal);
    tokenizer.map_special_lexical("<", TokenKind::OperatorLesser);
    tokenizer.map_special_lexical("<=", TokenKind::OperatorLesserEquals);
    tokenizer.map_special_lexical(">", TokenKind::OperatorGreater);
    tokenizer.map_special_lexical(">=", TokenKind::OperatorGreaterEquals);
    tokenizer.map_special_lexical("&&", TokenKind::OperatorAnd);
    tokenizer.map_special_lexical("||", TokenKind::OperatorOr);
    tokenizer.map_special_lexical("=", TokenKind::OperatorAssign);
    tokenizer.map_special_lexical("~", TokenKind::OperatorBitFlip);
    tokenizer.map_special_lexical("&", TokenKind::OperatorBitAnd);
    tokenizer.map_special_lexical("^", TokenKind::OperatorBitXor);
    tokenizer.map_special_lexical("|", TokenKind::OperatorBitOr);
    tokenizer.map_special_lexical("<<", TokenKind::OperatorBShiftLeft);
    tokenizer.map_special_lexical(">>", TokenKind::OperatorBShiftRight);
    tokenizer.map_special_lexical("undefined", TokenKind::LiteralUndefined);
    tokenizer.map_special_lexical("null", TokenKind::LiteralNull);
    tokenizer.map_special_lexical("NaN", TokenKind::LiteralNaN);
    tokenizer.map_special_lexical("Infinity", TokenKind::LiteralInfinity);
    tokenizer.map_special_lexical("true", TokenKind::LiteralTrue);
    tokenizer.map_special_lexical("false", TokenKind::LiteralFalse);

    let all_tokens = tokenizer.lex_all(&source_txt);

    let mut parser = Parser::new(&all_tokens, &source_txt);
    let decls = parser.parse_data();

    if decls.is_none() {
        return ExitCode::FAILURE;
    }

    let ast = AST {
        txt: source_txt,
        tokens: all_tokens,
        decls: decls.expect("Expected parsed JS decls at main.rs ~ line#97."),
        name: source_fpath.to_owned(),
    };

    let mut bc_emitter = Emitter::new(64, 256);

    let program = bc_emitter.emit_code(&ast);

    if program.is_none() {
        return ExitCode::FAILURE;
    }

    let program = program.expect("Expected fully compiled program at main.rs ~ line#111.");

    if option_allow_bc_dump {
        dump_bytecode(&program);
        return ExitCode::SUCCESS;
    }

    let mut vm_state = JSContext::new(DEFAULT_SHAPE_POPULATION, DEFAULT_JS_STACK_SIZE, DEFAULT_JS_RECUR_LIMIT, program);

    let vm_status = run_vm(&mut vm_state);

    println!("{}", vm_state.stack[0]);

    println!("Finish status: {vm_status}");

    if vm_status == EvalStatus::Ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
