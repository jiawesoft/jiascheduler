use crate::{Context, Value};
use crate::{Environment, eval};
#[allow(deprecated)]
use crate::Parser;
use crate::Result;
use proptest::prelude::*;
use test_log::test;

macro_rules! test_old {
    ($code:expr, $expected:expr) => {{
        check!($code, $expected)?;
    }};
}

macro_rules! test {
    ($name:ident, $code:expr$(, $expected:tt)+) => {
        #[test]
        fn $name() -> Result<()> {
            check!($code$(, $expected)+)
        }
    };
}

macro_rules! check {
    ($code:expr$(, $expected:tt)+) => {{
        let ctx = Context::default();
        let code = $code;
        let expected = format!($($expected)+);
        println!("{} => {} (expected)", code, expected);
        let result = eval(&code, &ctx).unwrap();
        println!("{} => {}", code, result);
        assert_eq!(result.to_string(), expected);
        Result::Ok(())
    }};
}

#[test]
fn arithmetic() -> Result<()> {
    test_old!("2 + 3", "5");
    test_old!("2.1 + 3.2", "5.300000000000001");
    test_old!("2 - 3", "-1");
    test_old!("2.1 - 3.2", "-1.1");
    test_old!("2 * 3", "6");
    test_old!("2.1 * 3.2", "6.720000000000001");
    test_old!("7 / 3", "2");
    test_old!("7.0 / 3.0", "2.3333333333333335");
    test_old!("7 % 3", "1");
    test_old!("2 ** 3", "8");
    test_old!("2.0 ** 3.0", "8");
    test_old!("2 ^ 3", "8");
    test_old!("2.0 ^ 3.0", "8");
    test_old!("1 == 1", "true");
    test_old!("1 == 2", "false");
    test_old!("1 != 2", "true");
    test_old!("1 != 1", "false");
    test_old!("(1 + 2) * 3", "9");
    test_old!("+2 + 3", "5");
    test_old!("+2.0 + 3.5", "5.5");
    test_old!("-2 + 3", "1");
    test_old!("-2.0 + 3.5", "1.5");
    Ok(())
}

test!(order_of_ops, "1 + 2 * 3 + 1", "8");
test!(is_true, "true", "true");
test!(not_1, "!true", "false");
test!(false_1, "false", "false");
test!(and_1, "true && true", "true");
test!(and_2, "true && false", "false");
test!(and_3, "false && true", "false");
test!(and_4, "true and true", "true");
test!(and_5, "false and true", "false");
test!(and_6, "false or true", "true");

test!(string_concat, r#""foo" + "bar""#, r#""foobar""#);
test!(string_contains, r#""foo" contains "o""#, "true");

#[test]
fn string() -> Result<()> {
    test_old!(r#""foo" contains "x""#, "false");
    test_old!(r#""foo" startsWith "f""#, "true");
    test_old!(r#""foo" startsWith "x""#, "false");
    test_old!(r#""foo" endsWith "o""#, "true");
    test_old!(r#""foo" endsWith "x""#, "false");
    test_old!(r#""foo" == "foo""#, "true");
    test_old!(r#""foo" == "bar""#, "false");
    test_old!(r#""foo" != "bar""#, "true");
    test_old!(r#""foo" != "foo""#, "false");
    test_old!(r#""bar" < "foo""#, "true");
    test_old!(r#""foo" < "foo""#, "false");
    test_old!(r#""foo" > "bar""#, "true");
    test_old!(r#""foo" > "foo""#, "false");
    test_old!(r#""bar" <= "foo""#, "true");
    test_old!(r#""foo" <= "foo""#, "true");
    test_old!(r#""bar" >= "foo""#, "false");
    test_old!(r#""foo" >= "foo""#, "true");
    test_old!(r#""foo" matches "^f""#, "true");
    test_old!(r#""foo" matches "^x""#, "false");
    test_old!(
        r#"`foo
bar`"#,
        r#""foo\nbar""#
    );
    Ok(())
}

test!(nil, "nil", "nil");
test!(comment_line, "1 // foo", "1");
test!(comment_block, r#"/*
foo
*/ 1"#, "1");

#[test]
fn logic() -> Result<()> {
    test_old!(r#"true && false"#, "false");
    test_old!(r#"true || false"#, "true");
    test_old!(r#"true == true"#, "true");
    test_old!(r#"true == false"#, "false");
    test_old!(r#"true != false"#, "true");
    test_old!(r#"true != true"#, "false");
    test_old!(r#"!true"#, "false");
    test_old!(r#"not true"#, "false");
    Ok(())
}

// test!(arr_complex, r#"first([["xx"]])[0]"#, r#""xx""#);
test!(arr_idx, r#"["foo", "bar"][0]"#, r#""foo""#);

#[test]
fn array() -> Result<()> {
    test_old!(r#"["foo","bar"]"#, r#"["foo", "bar"]"#);
    test_old!(r#""foo" in ["foo", "bar"]"#, "true");
    test_old!(r#""foo" in ["bar", "baz"]"#, "false");
    test_old!(r#"["foo", "bar"][1]"#, r#""bar""#);
    test_old!(r#"["foo", "bar"][2]"#, "nil");
    test_old!(r#"["foo", "bar"][-1]"#, r#""bar""#);
    test_old!(r#"["foo", "bar"][0:1]"#, r#"["foo"]"#);
    test_old!(r#"["foo", "bar"][0:2]"#, r#"["foo", "bar"]"#);
    test_old!(r#"["foo", "bar"][0:-1]"#, r#"["foo"]"#);
    test_old!(r#"["foo", "bar"][1:]"#, r#"["bar"]"#);
    test_old!(r#"["foo", "bar"][:1]"#, r#"["foo"]"#);
    test_old!(r#"["foo", "bar"][:]"#, r#"["foo", "bar"]"#);
    Ok(())
}

#[test]
fn map() -> Result<()> {
    test_old!(r#"{foo: "bar"}"#, r#"{{foo: "bar"}}"#);
    test_old!(r#"{foo: "bar"}.foo"#, r#""bar""#);
    test_old!(r#"{foo: "bar"}.baz"#, "nil");
    test_old!(r#"{foo: "bar"}["foo"]"#, r#""bar""#);
    test_old!(r#"{foo: "bar"}["baz"]"#, "nil");
    test_old!(r#"{foo: "bar"}?.foo"#, r#""bar""#);
    test_old!(r#"{foo: "bar"}?.bar?.foo"#, r#"nil"#);
    test_old!(r#""foo" in {foo: "bar"}"#, "true");
    test_old!(r#""bar" in {foo: "bar"}"#, "false");
    Ok(())
}

#[test]
fn context() -> Result<()> {
    let ctx = Context::from_iter([("Version".to_string(), "v1.0.0".to_string())]);
    assert_eq!(
        eval(r#"Version matches "^v\\d+\\.\\d+\\.\\d+""#, &ctx)?
            .to_string(),
        "true"
    );
    assert_eq!(eval(r#""Version" in $env"#, &ctx)?.to_string(), r#"true"#);
    assert_eq!(
        eval(r#""version" in $env"#, &ctx)?.to_string(),
        r#"false"#
    );
    assert_eq!(
        eval(r#"$env["Version"]"#, &ctx)?.to_string(),
        r#""v1.0.0""#
    );
    Ok(())
}

#[test]
fn functions() -> Result<()> {
    let x = "s";
    let mut env = Environment::new();
    env.add_function("add", |c| -> Result<Value> {
        eprintln!("{}", x);
        let mut sum = 0;
        for arg in c.args {
            if let Value::Number(n) = arg {
                sum += n;
            } else {
                return Err(format!("Invalid argument: {arg:?}").into());
            }
        }
        Ok(sum.into())
    });
    let ctx = Context::default();
    assert_eq!(env.eval("add(1, 2, 3)", &ctx)?.to_string(), "6");
    assert_eq!(env.eval("3 | add(1, 2)", &ctx)?.to_string(), "6");
    Ok(())
}

#[test]
#[allow(deprecated)]
fn functions_with_parser() -> Result<()> {
    let x = "s";
    let mut p = Parser::new();
    p.add_function("add", |c| -> Result<Value> {
        eprintln!("{}", x);
        let mut sum = 0;
        for arg in c.args {
            if let Value::Number(n) = arg {
                sum += n;
            } else {
                return Err(format!("Invalid argument: {arg:?}").into());
            }
        }
        Ok(sum.into())
    });
    let ctx = Context::default();
    assert_eq!(p.eval("add(1, 2, 3)", &ctx)?.to_string(), "6");
    assert_eq!(p.eval("3 | add(1, 2)", &ctx)?.to_string(), "6");
    Ok(())
}

#[test]
fn string_functions() -> Result<()> {
    test_old!("trim(\"  foo  \")", r#""foo""#);
    test_old!("trim(\"__foo__\", \"_\")", r#""foo""#);
    test_old!("trimPrefix(\"foo\", \"f\")", r#""oo""#);
    test_old!("trimSuffix(\"foo\", \"oo\")", r#""f""#);
    test_old!("upper(\"foo\")", r#""FOO""#);
    test_old!("lower(\"FOO\")", r#""foo""#);
    test_old!("split(\"foo,bar\", \",\")", r#"["foo", "bar"]"#);
    test_old!(
        r#"split("apple,orange,grape", ",", 2)"#,
        r#"["apple", "orange,grape"]"#
    );
    test_old!("splitAfter(\"foo,bar\", \",\")", r#"["foo,", "bar"]"#);
    test_old!(
        r#"splitAfter("apple,orange,grape", ",", 2)"#,
        r#"["apple,", "orange,grape"]"#
    );
    test_old!(
        "replace(\"foo bar foo\", \"foo\", \"baz\")",
        r#""baz bar baz""#
    );
    test_old!(r#"repeat("Hi", 2)"#, r#""HiHiHi""#);
    test_old!("indexOf(\"foo bar foo\", \"bar\")", "4");
    test_old!("lastIndexOf(\"foo bar foo\", \"foo\")", "8");
    test_old!(r#"hasPrefix("HelloWorld", "Hello")"#, "true");
    test_old!(r#"hasSuffix("HelloWorld", "World")"#, "true");
    Ok(())
}

#[test]
fn array_functions() -> Result<()> {
    // TODO:
    // test_old!(r#"[{type: 'foo', v: 1}, {}]"#, r#"[{{type: "foo", v: 1}}, {{type: "foo", v: 2}}, {{type: "bar", v: 3}}]"#);
    test_old!(r#"all([1, 2, 3], {# > 0})"#, "true");
    test_old!(r#"all([1, 2, 3], {# > 1})"#, "false");
    test_old!(r#"any([1, 2, 3], {# > 2})"#, "true");
    test_old!(r#"any([1, 2, 3], {# > 3})"#, "false");
    test_old!(r#"one([1, 2, 3], {# > 2})"#, "true");
    test_old!(r#"one([1, 2, 3], {# > 1})"#, "false");
    test_old!(r#"none([1, 2, 3], {# > 3})"#, "true");
    test_old!(r#"none([1, 2, 3], {# > 2})"#, "false");
    test_old!(r#"map([1, 2, 3], {# * 2})"#, "[2, 4, 6]");
    test_old!(r#"filter([1, 2, 3], {# % 2 == 0})"#, "[2]");
    test_old!(r#"find([1, 2, 3], {# % 2 == 0})"#, "2");
    test_old!(r#"findIndex([1, 2, 3], {# % 2 == 0})"#, "1");
    test_old!(r#"findLast([1, 2, 3], {# % 2 == 1})"#, "3");
    test_old!(r#"findLastIndex([1, 2, 3], {# % 2 == 1})"#, "2");
    test_old!(r#"[{type: 'foo', v: 1}, {type: 'foo', v: 2}, {type: 'bar', v: 3}]"#, r#"[{{type: "foo", v: 1}}, {{type: "foo", v: 2}}, {{type: "bar", v: 3}}]"#);
    test_old!(r#"groupBy([{type: 'foo', v: 1}, {type: 'foo', v: 2}, {type: 'bar', v: 3}], .type).foo"#, r#"[{{type: "foo", v: 1}}, {{type: "foo", v: 2}}]"#);
    Ok(())
}

#[test]
fn variables() -> Result<()> {
    test_old!("let x = 1; x", "1");
    Ok(())
}

#[test]
fn ternary() -> Result<()> {
    test_old!("true ? 1 : 2", "1");
    test_old!("false ? 1 : 2", "2");
    Ok(())
}

#[test]
fn nil_coalesce() -> Result<()> {
    test_old!("nil ?? 1", "1");
    test_old!("2 ?? 1", "2");
    Ok(())
}

#[test]
fn range() -> Result<()> {
    test_old!("1..3 == [1, 2, 3]", "true");
    Ok(())
}

#[test]
fn filter() -> Result<()> {
    test_old!("filter(0..9, {# % 2 == 0})", "[0, 2, 4, 6, 8]");
    Ok(())
}

#[test]
fn version_expressions() -> Result<()> {
    // https://github.com/jdx/mise/discussions/3944#discussion-7778007
    let ctx = Context::from_iter([("Version".to_string(), "1.0.0".to_string())]);
    let mut env = Environment::new();

    // mock semver function for testing
    env.add_function("semver", |c| -> Result<Value> {
        if c.args.len() != 1 {
            return Err("semver() expects 1 argument".to_string().into());
        }
        Ok(Value::Bool(true))
    });

    assert_eq!(
        env.eval(r#"Version in ["latest", "stable"]"#, &ctx)?.to_string(),
        "false"
    );
    assert_eq!(
        env.eval(r#"not (Version in ["latest", "stable"])"#, &ctx)?.to_string(),
        "true"
    );
    assert_eq!(
        env.eval(r#"(not (Version in ["latest", "stable"])) and semver("> 0.4.5")"#, &ctx)?.to_string(),
        "true"
    );
    assert_eq!(
        env.eval(r#"(not (Version in ["latest", "stable"])) && semver("> 0.4.5")"#, &ctx)?.to_string(),
        "true"
    );

    Ok(())
}

test!(precedence_unary_vs_exponentiation, "-2 ** 4", "-16");

test!(
    precedence_unary_vs_exponentiation_grouped,
    "(-2) ** 4",
    "16"
);

test!(
    precedence_exponentiation_associativity,
    "2 ** 3 ** 2",
    "512"
);
test!(
    precedence_exponentiation_associativity_grouped,
    "(2 ** 3) ** 2",
    "64"
);

test!(precedence_mixed_arithmetic, "10 + 5 * 2 ** 3 - 1", "49");

test!(
    precedence_logical_or_vs_and,
    "true || false && false",
    "true"
);

test!(
    precedence_not_vs_and,
    "not true and true",
    "false"
);

test!(precedence_ternary_is_lowest, "5 > 10 ? 1 + 1 : 2 * 2", "4");
test!(
    precedence_ternary_associativity,
    "false ? 1 : true ? 2 : 3",
    "2"
);
test!(
    precedence_ternary_associativity_2,
    "false ? 1 : false ? 2 : 3",
    "3"
);

// TODO: implement type conversion functions
// test!(precedence_pipe_is_low, r#""a" == "a" | string()"#, r#""true""#);

test!(
    precedence_string_op_vs_and,
    r#""foo" contains "f" and "bar" startsWith "b""#,
    "true"
);

test!(
    precedence_string_op_vs_or,
    r#""foo" endsWith "x" or "bar" contains "a""#,
    "true"
);

test!(
    precedence_in_op_vs_and,
    r#"1 in [1, 2] and 3 in [3, 4]"#,
    "true"
);

test!(
    precedence_in_op_vs_or,
    r#"1 in [2, 3] or 3 in [3, 4]"#,
    "true"
);

test!(
    precedence_matches_vs_and,
    r#""foo123" matches "^1" and "123foo" matches "^1""#,
    "false"
);

test!(
    precedence_op_and_or_chain_1,
    r#""a" == "b" or "c" == "c" and "d" == "f""#,
    "false"
);

test!(
    precedence_op_and_or_chain_2,
    r#""a" startsWith "x" or "b" contains "b" and "c" endsWith "c""#,
    "true"
);

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    #[test]
    fn test_addition(a in -100000..100000, b in -100000..100000) {
        let sum = a + b;
        check!(format!("{a} + {b}"), "{sum}").unwrap()
    }
}
