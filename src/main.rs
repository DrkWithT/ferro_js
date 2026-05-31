pub mod frontend;

use std::{env::args, fs::read_to_string, process::ExitCode};

use crate::frontend::{lexer, parser::Parser, token::{Token, TokenKind}};


const RUNTIME_NAME: &str = "    ______                    \n   / ____/__  ______________  \n  / /_  / _ \\/ ___/ ___/ __ \\ \n / __/ /  __/ /  / /  / /_/ / \n/_/    \\___/_/  /_/   \\____/ \n";

fn main() -> ExitCode {
    let mut file_name: Option<String> = None;
    let mut option_show_version = false;
    let mut option_show_help = false;

    for arg in args().skip(1) {
        match arg.as_str() {
            "-v" => { option_show_version = true; },
            "-h" => { option_show_help = true; },
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

    let mut tokenizer = lexer::Lexer::new(&source_txt);
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
    tokenizer.map_special_lexical("<", TokenKind::OperatorLesser);
    tokenizer.map_special_lexical("<=", TokenKind::OperatorLesserEquals);
    tokenizer.map_special_lexical(">", TokenKind::OperatorGreater);
    tokenizer.map_special_lexical(">=", TokenKind::OperatorGreaterEquals);
    tokenizer.map_special_lexical("&&", TokenKind::OperatorAnd);
    tokenizer.map_special_lexical("||", TokenKind::OperatorOr);
    tokenizer.map_special_lexical("=", TokenKind::OperatorAssign);
    tokenizer.map_special_lexical("|", TokenKind::OperatorBitOr);
    tokenizer.map_special_lexical("&", TokenKind::OperatorBitAnd);
    tokenizer.map_special_lexical("<<", TokenKind::OperatorBShiftLeft);
    tokenizer.map_special_lexical(">>", TokenKind::OperatorBShiftRight);
    tokenizer.map_special_lexical("undefined", TokenKind::LiteralUndefined);
    tokenizer.map_special_lexical("null", TokenKind::LiteralNull);
    tokenizer.map_special_lexical("NaN", TokenKind::LiteralNaN);
    tokenizer.map_special_lexical("Infinity", TokenKind::LiteralInfinity);
    tokenizer.map_special_lexical("true", TokenKind::LiteralTrue);
    tokenizer.map_special_lexical("false", TokenKind::LiteralFalse);

    let all_tokens = tokenizer.lex_all(&source_txt);

    for (pos, Token {begin, end, line, kind}) in all_tokens.iter().enumerate() {
        println!("Token #{pos}:\n\t[begin = {}, end = {}, line = {}, kind = {}", *begin, *end, *line, *kind);
    }

    let mut parser = Parser::new(&all_tokens, &source_txt);
    let _ = parser.parse_data();

    ExitCode::SUCCESS
}
