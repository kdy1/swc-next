use std::borrow::Cow;

use swc_next_allocator::Allocator;
use swc_next_ecma_parser::{NoTokenParserConfig, Options as ParserOptions, Parser};
use swc_next_type_strip::{Lang, Options, Severity, SourceType, Span, StripResult, strip_types};

fn options(lang: Lang) -> Options {
    Options {
        source_type: SourceType::Unambiguous,
        lang,
        ..Options::default()
    }
}

#[track_caller]
fn assert_utf16_len_preserved(input: &str, output: &str) {
    assert_eq!(output.encode_utf16().count(), input.encode_utf16().count());
}

#[track_caller]
fn assert_valid_javascript(source: &str, lang: Lang) {
    let lang = if lang == Lang::Tsx {
        Lang::Jsx
    } else {
        Lang::Js
    };
    let allocator = Allocator::default();
    let parser_options = ParserOptions {
        source_type: SourceType::Unambiguous,
        lang,
        ..ParserOptions::default()
    };
    let parsed = Parser::init(&allocator, source, parser_options, NoTokenParserConfig).parse();
    assert!(parsed.diagnostics.is_empty());
}

#[track_caller]
fn assert_strip_text<'a>(input: &'a str, expected: &str, lang: Lang) -> StripResult<'a> {
    let checked = options(lang);
    let unchecked = Options {
        skip_erasable_syntax_check: true,
        ..checked
    };
    let result = strip_types(input, checked);
    assert_utf16_len_preserved(input, &result.code);
    assert_eq!(result.code, expected);
    assert!(result.diagnostics.is_empty());

    let unchecked_result = strip_types(input, unchecked);
    assert_eq!(unchecked_result, result);
    result
}

#[track_caller]
fn assert_strip(input: &str, expected: &str) {
    let result = assert_strip_text(input, expected, Lang::Ts);
    assert_valid_javascript(&result.code, Lang::Ts);
}

#[track_caller]
fn assert_strip_tsx(input: &str, expected: &str) {
    let result = assert_strip_text(input, expected, Lang::Tsx);
    assert_valid_javascript(&result.code, Lang::Tsx);
}

#[track_caller]
fn assert_strip_error<'a>(input: &'a str, expected: &str) -> StripResult<'a> {
    let checked = options(Lang::Ts);
    let unchecked = Options {
        skip_erasable_syntax_check: true,
        ..checked
    };
    let result = strip_types(input, checked);
    assert_utf16_len_preserved(input, &result.code);
    assert_eq!(result.code, expected);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, Severity::Error);
    assert_valid_javascript(&result.code, Lang::Ts);

    let unchecked_result = strip_types(input, unchecked);
    assert_eq!(unchecked_result.code, result.code);
    assert!(unchecked_result.diagnostics.is_empty());
    result
}

#[track_caller]
fn assert_strip_error_at<'a>(
    input: &'a str,
    expected: &str,
    diagnostic_text: &str,
) -> StripResult<'a> {
    let result = assert_strip_error(input, expected);
    let Span { start, end } = result.diagnostics[0].span;
    assert_eq!(
        input.get(start as usize..end as usize),
        Some(diagnostic_text)
    );
    result
}

mod annotations;
mod asi;
mod bindings;
mod classes;
mod declarations;
mod diagnostics;
mod expressions;
mod generics;
mod modules;

#[test]
fn unchanged_source_returns_borrowed_code() {
    let source = "const value = 1;";
    let result = strip_types(source, options(Lang::Ts));

    assert!(matches!(result.code, Cow::Borrowed(value) if value == source));
    assert!(result.diagnostics.is_empty());
    assert_valid_javascript(&result.code, Lang::Ts);
}

#[test]
fn stripped_source_returns_owned_code() {
    let result = strip_types("let value: number = 1;", options(Lang::Ts));

    assert!(matches!(result.code, Cow::Owned(_)));
    assert_valid_javascript(&result.code, Lang::Ts);
}

#[test]
fn parser_error_returns_borrowed_code() {
    let source = "let value: = 1;";
    let checked = options(Lang::Ts);
    let unchecked = Options {
        skip_erasable_syntax_check: true,
        ..checked
    };
    for options in [checked, unchecked] {
        let result = strip_types(source, options);

        assert!(matches!(result.code, Cow::Borrowed(value) if value == source));
        assert!(!result.diagnostics.is_empty());
    }
}

#[test]
fn parser_error_stops_processing() {
    let source = "let a:T;\nlet b:=1;\nlet c:=2;";
    let result = strip_types(source, options(Lang::Ts));

    assert!(matches!(result.code, Cow::Borrowed(value) if value == source));
    assert_eq!(result.diagnostics.len(), 1);
}

#[test]
fn language() {
    let source = "const element = <Component<Type> />;";
    let result = strip_types(source, options(Lang::Ts));
    assert!(!result.diagnostics.is_empty());

    let result = strip_types(source, options(Lang::Tsx));
    assert!(result.diagnostics.is_empty());
    assert_valid_javascript(&result.code, Lang::Tsx);
}

#[test]
fn parentheses() {
    assert_strip("(value) as Type;", "(value)        ;");
}

#[test]
fn comments() {
    assert_strip(
        "/* lead */ let value: number = 1; // tail",
        "/* lead */ let value         = 1; // tail",
    );
}

#[test]
fn unsupported_lang_variants() {
    let checked = options(Lang::Js);
    let unchecked = Options {
        skip_erasable_syntax_check: true,
        ..checked
    };
    for mut options in [checked, unchecked] {
        let source = "const value = 1;";
        let result = strip_types(source, options);
        assert!(matches!(result.code, Cow::Borrowed(value) if value == source));
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);

        options.lang = Lang::Jsx;
        let result = strip_types(source, options);
        assert!(matches!(result.code, Cow::Borrowed(value) if value == source));
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);

        let source = "declare const value: number;";
        options.lang = Lang::Dts;
        let result = strip_types(source, options);
        assert!(matches!(result.code, Cow::Borrowed(value) if value == source));
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }
}
