/// Construct an `expr::Value` from a json-like literal.
///
/// Compared to json:
///
/// * Trailing commas are allowed
/// * `nil` is used instead of `null`
///
/// ```
/// let value = expr::value!({
///     "code": 200,
///     "success": true,
///     "payload": {
///         "features": [
///             "expr",
///             "macro",
///         ],
///         "homepage": nil,
///     },
/// });
/// ```
///
/// Variables or expressions can be interpolated into the literal. Any type
/// interpolated into an array element or map value must implement Into<Value>,
/// while any type interpolated into a map key must implement `Into<String>`.
/// If the interpolated type contains a map with non-string keys, the `value!`
/// macro will panic.
///
/// ```
/// let code = 200;
/// let features = vec!["expr", "macro"];
///
/// let value = expr::value!({
///     "code": code,
///     "success": code == 200,
///     "payload": {
///         features[0]: features[1],
///     },
/// });
/// ```
#[macro_export]
macro_rules! value {
    //////////////////////////////////////////////////////////////////////////
    // TT muncher for parsing the inside of an array [...]. Produces a vec![...]
    // of the elements.
    //
    // Must be invoked as: value!(@array [] $($tt)*)
    //////////////////////////////////////////////////////////////////////////

    // Done with trailing comma.
    (@array [$($elems:expr,)*]) => {
        vec![$($elems,)*]
    };

    // Done without trailing comma.
    (@array [$($elems:expr),*]) => {
        vec![$($elems),*]
    };

    // Next element is `nil`.
    (@array [$($elems:expr,)*] nil $($rest:tt)*) => {
        $crate::value!(@array [$($elems,)* $crate::value!(nil)] $($rest)*)
    };

    // Next element is `true`.
    (@array [$($elems:expr,)*] true $($rest:tt)*) => {
        $crate::value!(@array [$($elems,)* $crate::value!(true)] $($rest)*)
    };

    // Next element is `false`.
    (@array [$($elems:expr,)*] false $($rest:tt)*) => {
        $crate::value!(@array [$($elems,)* $crate::value!(false)] $($rest)*)
    };

    // Next element is an array.
    (@array [$($elems:expr,)*] [$($array:tt)*] $($rest:tt)*) => {
        $crate::value!(@array [$($elems,)* $crate::value!([$($array)*])] $($rest)*)
    };

    // Next element is a map.
    (@array [$($elems:expr,)*] {$($map:tt)*} $($rest:tt)*) => {
        $crate::value!(@array [$($elems,)* $crate::value!({$($map)*})] $($rest)*)
    };

    // Next element is an expression followed by comma.
    (@array [$($elems:expr,)*] $next:expr, $($rest:tt)*) => {
        $crate::value!(@array [$($elems,)* $crate::value!($next),] $($rest)*)
    };

    // Last element is an expression with no trailing comma.
    (@array [$($elems:expr,)*] $last:expr) => {
        $crate::value!(@array [$($elems,)* $crate::value!($last)])
    };

    // Comma after the most recent element.
    (@array [$($elems:expr),*] , $($rest:tt)*) => {
        $crate::value!(@array [$($elems,)*] $($rest)*)
    };

    // Unexpected token after most recent element.
    (@array [$($elems:expr),*] $unexpected:tt $($rest:tt)*) => {
        $crate::value_unexpected!($unexpected)
    };

    //////////////////////////////////////////////////////////////////////////
    // TT muncher for parsing the inside of an map {...}. Each entry is
    // inserted into the given map variable.
    //
    // Must be invoked as: value!(@map $map () ($($tt)*) ($($tt)*))
    //
    // We require two copies of the input tokens so that we can match on one
    // copy and trigger errors on the other copy.
    //////////////////////////////////////////////////////////////////////////

    // Done.
    (@map $map:ident () () ()) => {};

    // Insert the current entry followed by trailing comma.
    (@map $map:ident [$($key:tt)+] ($value:expr) , $($rest:tt)*) => {
        let _ = $map.insert(($($key)+).into(), $value);
        $crate::value!(@map $map () ($($rest)*) ($($rest)*));
    };

    // Current entry followed by unexpected token.
    (@map $map:ident [$($key:tt)+] ($value:expr) $unexpected:tt $($rest:tt)*) => {
        $crate::value_unexpected!($unexpected);
    };

    // Insert the last entry without trailing comma.
    (@map $map:ident [$($key:tt)+] ($value:expr)) => {
        let _ = $map.insert(($($key)+).into(), $value);
    };

    // Next value is `nil`.
    (@map $map:ident ($($key:tt)+) (: nil $($rest:tt)*) $copy:tt) => {
        $crate::value!(@map $map [$($key)+] ($crate::value!(nil)) $($rest)*);
    };

    // Next value is `true`.
    (@map $map:ident ($($key:tt)+) (: true $($rest:tt)*) $copy:tt) => {
        $crate::value!(@map $map [$($key)+] ($crate::value!(true)) $($rest)*);
    };

    // Next value is `false`.
    (@map $map:ident ($($key:tt)+) (: false $($rest:tt)*) $copy:tt) => {
        $crate::value!(@map $map [$($key)+] ($crate::value!(false)) $($rest)*);
    };

    // Next value is an array.
    (@map $map:ident ($($key:tt)+) (: [$($array:tt)*] $($rest:tt)*) $copy:tt) => {
        $crate::value!(@map $map [$($key)+] ($crate::value!([$($array)*])) $($rest)*);
    };

    // Next value is a map.
    (@map $map:ident ($($key:tt)+) (: {$($inner:tt)*} $($rest:tt)*) $copy:tt) => {
        $crate::value!(@map $map [$($key)+] ($crate::value!({$($inner)*})) $($rest)*);
    };

    // Next value is an expression followed by comma.
    (@map $map:ident ($($key:tt)+) (: $value:expr , $($rest:tt)*) $copy:tt) => {
        $crate::value!(@map $map [$($key)+] ($crate::value!($value)) , $($rest)*);
    };

    // Last value is an expression with no trailing comma.
    (@map $map:ident ($($key:tt)+) (: $value:expr) $copy:tt) => {
        $crate::value!(@map $map [$($key)+] ($crate::value!($value)));
    };

    // Missing value for last entry. Trigger a reasonable error message.
    (@map $map:ident ($($key:tt)+) (:) $copy:tt) => {
        // "unexpected end of macro invocation"
        $crate::value!();
    };

    // Missing colon and value for last entry. Trigger a reasonable error
    // message.
    (@map $map:ident ($($key:tt)+) () $copy:tt) => {
        // "unexpected end of macro invocation"
        $crate::value!();
    };

    // Misplaced colon. Trigger a reasonable error message.
    (@map $map:ident () (: $($rest:tt)*) ($colon:tt $($copy:tt)*)) => {
        // Takes no arguments so "no rules expected the token `:`".
        $crate::value_unexpected!($colon);
    };

    // Found a comma inside a key. Trigger a reasonable error message.
    (@map $map:ident ($($key:tt)*) (, $($rest:tt)*) ($comma:tt $($copy:tt)*)) => {
        // Takes no arguments so "no rules expected the token `,`".
        $crate::value_unexpected!($comma);
    };

    // Key is fully parenthesized. This avoids clippy double_parens false
    // positives because the parenthesization may be necessary here.
    (@map $map:ident () (($key:expr) : $($rest:tt)*) $copy:tt) => {
        $crate::value!(@map $map ($key) (: $($rest)*) (: $($rest)*));
    };

    // Refuse to absorb colon token into key expression.
    (@map $map:ident ($($key:tt)*) (: $($unexpected:tt)+) $copy:tt) => {
        $crate::value_expect_expr_comma!($($unexpected)+);
    };

    // Munch a token into the current key.
    (@map $map:ident ($($key:tt)*) ($tt:tt $($rest:tt)*) $copy:tt) => {
        $crate::value!(@map $map ($($key)* $tt) ($($rest)*) ($($rest)*));
    };

    //////////////////////////////////////////////////////////////////////////
    // The main implementation.
    //
    // Must be invoked as: value!($($data)+)
    //////////////////////////////////////////////////////////////////////////

    (nil) => {
        $crate::Value::Nil
    };

    (true) => {
        $crate::Value::Bool(true)
    };

    (false) => {
        $crate::Value::Bool(false)
    };

    ([]) => {
        $crate::Value::Array(vec![])
    };

    ([ $($tt:tt)+ ]) => {
        $crate::Value::Array($crate::value!(@array [] $($tt)+))
    };

    ({}) => {
        $crate::Value::Map($crate::__private::IndexMap::new())
    };

    ({ $($tt:tt)+ }) => {
        $crate::Value::Map({
            let mut map: $crate::__private::IndexMap<String, $crate::Value> = $crate::__private::IndexMap::new();
            $crate::value!(@map map () ($($tt)+) ($($tt)+));
            map
        })
    };

    // Any type that implements Into<Value>: numbers, strings, variables etc.
    // Must be below every other rule.
    ($other:expr) => {
        $crate::Value::from($other)
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! value_unexpected {
    () => {};
}

#[macro_export]
#[doc(hidden)]
macro_rules! value_expect_expr_comma {
    ($e:expr , $($tt:tt)*) => {};
}

#[cfg(test)]
mod tests {
    use crate::Value;
    use indexmap::IndexMap;

    #[test]
    fn test_value_macro_boolean() {
        let true_val = value!(true);
        assert_eq!(true_val, Value::Bool(true));

        let false_val = value!(false);
        assert_eq!(false_val, Value::Bool(false));
    }

    #[test]
    fn test_value_macro_string() {
        let string_val = value!("foo");
        assert_eq!(string_val, Value::String("foo".to_string()));

        let empty_string = value!("");
        assert_eq!(empty_string, Value::String("".to_string()));
    }

    #[test]
    fn test_value_macro_nil() {
        let nil_val = value!(nil);
        assert_eq!(nil_val, Value::Nil);
    }

    #[test]
    fn test_value_macro_numeric() {
        let integer_val = value!(42);
        assert_eq!(integer_val, Value::Number(42));

        let float_val = value!(3.14);
        assert_eq!(float_val, Value::Float(3.14));

        let negative_val = value!(-10.5);
        assert_eq!(negative_val, Value::Float(-10.5));
    }

    #[test]
    fn test_value_macro_array() {
        let empty_array = value!([]);
        assert_eq!(empty_array, Value::Array(vec![]));

        let simple_array = value!([1, 2, 3]);
        assert_eq!(
            simple_array,
            Value::Array(vec![
                Value::Number(1),
                Value::Number(2),
                Value::Number(3),
            ])
        );

        let mixed_array = value!([true, "hello", nil, 42.5]);
        assert_eq!(
            mixed_array,
            Value::Array(vec![
                Value::Bool(true),
                Value::String("hello".to_string()),
                Value::Nil,
                Value::Float(42.5),
            ])
        );

        let nested_array = value!([[1, 2], ["a", "b"]]);
        assert_eq!(
            nested_array,
            Value::Array(vec![
                Value::Array(vec![Value::Number(1), Value::Number(2)]),
                Value::Array(vec![
                    Value::String("a".to_string()),
                    Value::String("b".to_string()),
                ]),
            ])
        );
    }

    #[test]
    fn test_value_macro_map() {
        let empty_map = value!({});
        assert_eq!(empty_map, Value::Map(IndexMap::new()));

        let simple_map = value!({
            "key1": "value1",
            "key2": 42,
        });
        let expected_simple_map = Value::from_iter([
            ("key1", Value::from("value1")),
            ("key2", Value::from(42)),
        ]);
        assert_eq!(simple_map, expected_simple_map);

        let complex_map = value!({
            "boolean": true,
            "string": "hello",
            "nil": nil,
            "number": 3.14,
            "array": [1, 2, 3],
            "nested_map": {
                "inner_key": "inner_value",
            },
        });
        let expected_complex_map = Value::from_iter([
            ("boolean", Value::Bool(true)),
            ("string", Value::String("hello".to_string())),
            ("nil", Value::Nil),
            ("number", Value::Float(3.14)),
            ("array", Value::from(vec![
                Value::Number(1),
                Value::Number(2),
                Value::Number(3),
            ])),
            ("nested_map", Value::from_iter([
                ("inner_key", Value::String("inner_value".to_string()))
            ])),
        ]);
        assert_eq!(complex_map, expected_complex_map);
    }

    #[test]
    fn test_interpolation() {
        let bool_var = true;
        let string_var = "hello".to_string();
        let int_var = 42;
        let float_var = 3.14;
        let nil_var = Value::Nil;
        let array_var = vec![1, 2, 3];
        let map_var: IndexMap<String, Value> = IndexMap::from([
            ("key".to_string(), Value::String("value".to_string())),
        ]);

        let interpolated_value = value!({
            "bool": bool_var,
            "string": string_var,
            "int": int_var,
            "float": float_var,
            "nil": nil_var,
            "array": array_var,
            "map": map_var,
        });
        let expected_value = Value::from_iter([
            ("bool", Value::from(true)),
            ("string", Value::from("hello")),
            ("int", Value::from(42)),
            ("float", Value::from(3.14)),
            ("nil", Value::Nil),
            ("array", Value::from(vec![
                Value::from(1),
                Value::from(2),
                Value::from(3),
            ])),
            ("map", Value::from_iter([
                ("key", Value::from("value")),
            ])),
        ]);
        assert_eq!(interpolated_value, expected_value);
    }
}
