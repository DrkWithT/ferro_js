pub mod frontend;
pub mod backend;
pub mod runtime;
pub mod core;

use std::{env::args, fs::read_to_string, process::ExitCode};

use crate::{
    core::driver::{ Driver, DriverConfig, DriverFlag },
    frontend::{ token::TokenKind },
    runtime::{ code::JSGlobalConstID, ctx::EvalStatus }
};

fn main() -> ExitCode {
    let mut file_name: Option<String> = None;
    let mut option_bits = 0u8;

    for arg in args().skip(1) {
        match arg.as_str() {
            "-v" => { option_bits |= DriverFlag::ShowVersion as u8 },
            "-h" => { option_bits |= DriverFlag::ShowHelp as u8 },
            "-d" => { option_bits |= DriverFlag::DumpBytecode as u8 },
            "-r" => { option_bits |= DriverFlag::RunScript as u8 },
            _ => file_name = Some(arg)
        }
    }

    let source_fpath = String::from("main.js");
    let source_fpath = file_name.as_ref().unwrap_or(&source_fpath).as_str();
    let source_txt = read_to_string(source_fpath);

    let source_txt = source_txt.unwrap_or_default();

    let mut driver = Driver::new(DriverConfig::default());

    driver.add_lexical("var", TokenKind::KeywordVar);
    driver.add_lexical("if", TokenKind::KeywordIf);
    driver.add_lexical("else", TokenKind::KeywordElse);
    driver.add_lexical("while", TokenKind::KeywordWhile);
    driver.add_lexical("for", TokenKind::KeywordFor);
    driver.add_lexical("return", TokenKind::KeywordReturn);
    driver.add_lexical("function", TokenKind::KeywordFunction);
    driver.add_lexical("new", TokenKind::KeywordNew);
    driver.add_lexical("this", TokenKind::KeywordThis);
    driver.add_lexical("typeof", TokenKind::KeywordTypeOf);
    driver.add_lexical("void", TokenKind::KeywordVoid);
    driver.add_lexical("delete", TokenKind::KeywordDelete);
    driver.add_lexical("get", TokenKind::KeywordGet);
    driver.add_lexical("set", TokenKind::KeywordSet);
    driver.add_lexical("++", TokenKind::OperatorPlusPlus);
    driver.add_lexical("--", TokenKind::OperatorMinusMinus);
    driver.add_lexical("!", TokenKind::OperatorBang);
    driver.add_lexical("%", TokenKind::OperatorPct);
    driver.add_lexical("*", TokenKind::OperatorTimes);
    driver.add_lexical("/", TokenKind::OperatorSlash);
    driver.add_lexical("+", TokenKind::OperatorPlus);
    driver.add_lexical("-", TokenKind::OperatorMinus);
    driver.add_lexical("===", TokenKind::OperatorStrictEquals);
    driver.add_lexical("!==", TokenKind::OperatorStrictUnequals);
    driver.add_lexical("==", TokenKind::OperatorLooseEqual);
    driver.add_lexical("!=", TokenKind::OperatorLooseUnequal);
    driver.add_lexical("<", TokenKind::OperatorLesser);
    driver.add_lexical("<=", TokenKind::OperatorLesserEquals);
    driver.add_lexical(">", TokenKind::OperatorGreater);
    driver.add_lexical(">=", TokenKind::OperatorGreaterEquals);
    driver.add_lexical("&&", TokenKind::OperatorAnd);
    driver.add_lexical("||", TokenKind::OperatorOr);
    driver.add_lexical("=", TokenKind::OperatorAssign);
    driver.add_lexical("~", TokenKind::OperatorBitFlip);
    driver.add_lexical("&", TokenKind::OperatorBitAnd);
    driver.add_lexical("^", TokenKind::OperatorBitXor);
    driver.add_lexical("|", TokenKind::OperatorBitOr);
    driver.add_lexical("<<", TokenKind::OperatorBShiftLeft);
    driver.add_lexical(">>", TokenKind::OperatorBShiftRight);
    driver.add_lexical("undefined", TokenKind::LiteralUndefined);
    driver.add_lexical("null", TokenKind::LiteralNull);
    driver.add_lexical("NaN", TokenKind::LiteralNaN);
    driver.add_lexical("Infinity", TokenKind::LiteralInfinity);
    driver.add_lexical("true", TokenKind::LiteralTrue);
    driver.add_lexical("false", TokenKind::LiteralFalse);

    driver.add_global_string(JSGlobalConstID::TypenameUndefined, "undefined");
    driver.add_global_string(JSGlobalConstID::TypenameBoolean, "boolean");
    driver.add_global_string(JSGlobalConstID::TypenameNumber, "number");
    driver.add_global_string(JSGlobalConstID::TypenameString, "string");
    driver.add_global_string(JSGlobalConstID::TypenameObject, "object");
    driver.add_global_string(JSGlobalConstID::TypenameFunction, "function");

    let (ferro_result, ferro_status) = driver.run(&source_txt, option_bits);

    println!("{ferro_result}, {ferro_status}");

    if ferro_status == EvalStatus::Ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
