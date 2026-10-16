use crate::error::LuaResult;
use crate::interp::Interp;
use crate::stdlib::{check_number, function_table, optional_integer};
use crate::value::{TableRef, Value};
pub fn library() -> TableRef {
    let table = function_table([
        ("abs", 1, lua_abs as crate::value::NativeFunction),
        ("acos", 1, lua_acos),
        ("asin", 1, lua_asin),
        ("atan", 1, lua_atan),
        ("atan2", 2, lua_atan2),
        ("ceil", 1, lua_ceil),
        ("cos", 1, lua_cos),
        ("cosh", 1, lua_cosh),
        ("deg", 1, lua_deg),
        ("exp", 1, lua_exp),
        ("floor", 1, lua_floor),
        ("fmod", 2, lua_fmod),
        ("frexp", 1, lua_frexp),
        ("ldexp", 2, lua_ldexp),
        ("log", 1, lua_log),
        ("log10", 1, lua_log10),
        ("max", 1, lua_max),
        ("min", 1, lua_min),
        ("modf", 1, lua_modf),
        ("pow", 2, lua_pow),
        ("rad", 1, lua_rad),
        ("random", 0, lua_random),
        ("randomseed", 1, lua_randomseed),
        ("sin", 1, lua_sin),
        ("sinh", 1, lua_sinh),
        ("sqrt", 1, lua_sqrt),
        ("tan", 1, lua_tan),
        ("tanh", 1, lua_tanh),
    ]);
    table.set(Value::from("pi"), Value::Number(std::f64::consts::PI));
    table.set(Value::from("huge"), Value::Number(f64::INFINITY));
    table.set(Value::from("maxinteger"), Value::Number(i64::MAX as f64));
    table.set(Value::from("mininteger"), Value::Number(i64::MIN as f64));
    table
}

thread_local! {
    static RANDOM_STATE: core::cell::Cell<u64> = const { core::cell::Cell::new(0x2545_F491_4F6C_DD1D) };
}

fn next_random() -> u64 {
    RANDOM_STATE.with(|state| {
        let mut value = state.get();
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        state.set(value);
        value.wrapping_mul(0x2545_F491_4F6C_DD1D)
    })
}

fn random_unit() -> f64 {
    (next_random() >> 11) as f64 / (1u64 << 53) as f64
}

fn lua_abs(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    Ok(vec![Value::Number(
        check_number(interp, arguments, 0)?.abs(),
    )])
}

macro_rules! unary_math {
    ($name:ident, $method:ident) => {
        fn $name(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
            Ok(vec![Value::Number(
                check_number(interp, arguments, 0)?.$method(),
            )])
        }
    };
}

unary_math!(lua_acos, acos);
unary_math!(lua_asin, asin);
unary_math!(lua_atan, atan);
unary_math!(lua_cos, cos);
unary_math!(lua_cosh, cosh);
unary_math!(lua_exp, exp);
unary_math!(lua_log, ln);
unary_math!(lua_log10, log10);
unary_math!(lua_sin, sin);
unary_math!(lua_sinh, sinh);
unary_math!(lua_sqrt, sqrt);
unary_math!(lua_tan, tan);
unary_math!(lua_tanh, tanh);

fn lua_atan2(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let y = check_number(interp, arguments, 0)?;
    let x = check_number(interp, arguments, 1)?;
    Ok(vec![Value::Number(y.atan2(x))])
}

fn lua_ceil(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    Ok(vec![Value::Number(
        check_number(interp, arguments, 0)?.ceil(),
    )])
}

fn lua_floor(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    Ok(vec![Value::Number(
        check_number(interp, arguments, 0)?.floor(),
    )])
}

fn lua_deg(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    Ok(vec![Value::Number(
        check_number(interp, arguments, 0)?.to_degrees(),
    )])
}

fn lua_rad(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    Ok(vec![Value::Number(
        check_number(interp, arguments, 0)?.to_radians(),
    )])
}

fn lua_fmod(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let a = check_number(interp, arguments, 0)?;
    let b = check_number(interp, arguments, 1)?;
    Ok(vec![Value::Number(a % b)])
}

fn lua_pow(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let a = check_number(interp, arguments, 0)?;
    let b = check_number(interp, arguments, 1)?;
    Ok(vec![Value::Number(a.powf(b))])
}

fn lua_frexp(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let value = check_number(interp, arguments, 0)?;
    if value == 0.0 || !value.is_finite() {
        return Ok(vec![Value::Number(value), Value::Number(0.0)]);
    }
    let exponent = value.abs().log2().floor() as i32 + 1;
    let mantissa = value / 2f64.powi(exponent);
    Ok(vec![
        Value::Number(mantissa),
        Value::Number(f64::from(exponent)),
    ])
}

fn lua_ldexp(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let mantissa = check_number(interp, arguments, 0)?;
    let exponent = optional_integer(interp, arguments, 1, 0)?;
    Ok(vec![Value::Number(mantissa * 2f64.powi(exponent as i32))])
}

fn lua_modf(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let value = check_number(interp, arguments, 0)?;
    let integral = value.trunc();
    Ok(vec![
        Value::Number(integral),
        Value::Number(value - integral),
    ])
}

fn lua_max(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    if arguments.is_empty() {
        return Err(interp.raise_str("bad argument #1 (number expected, got no value)"));
    }
    let mut best = check_number(interp, arguments, 0)?;
    for index in 1..arguments.len() {
        let value = check_number(interp, arguments, index)?;
        if value > best {
            best = value;
        }
    }
    Ok(vec![Value::Number(best)])
}

fn lua_min(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    if arguments.is_empty() {
        return Err(interp.raise_str("bad argument #1 (number expected, got no value)"));
    }
    let mut best = check_number(interp, arguments, 0)?;
    for index in 1..arguments.len() {
        let value = check_number(interp, arguments, index)?;
        if value < best {
            best = value;
        }
    }
    Ok(vec![Value::Number(best)])
}

fn lua_random(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let unit = random_unit();
    match arguments.len() {
        0 => Ok(vec![Value::Number(unit)]),
        1 => {
            let upper = interp.to_integer(&arguments[0])?;
            if upper < 1 {
                return Err(interp.raise_str("bad argument #1 (interval is empty)"));
            }
            let value = (unit * upper as f64).floor() as i64 + 1;
            Ok(vec![Value::Number(value.min(upper) as f64)])
        }
        _ => {
            let lower = interp.to_integer(&arguments[0])?;
            let upper = interp.to_integer(&arguments[1])?;
            if lower > upper {
                return Err(interp.raise_str("bad argument #2 (interval is empty)"));
            }
            let span = (upper - lower + 1) as f64;
            let value = lower + (unit * span).floor() as i64;
            Ok(vec![Value::Number(value.min(upper) as f64)])
        }
    }
}

fn lua_randomseed(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let seed = check_number(interp, arguments, 0)?;

    let mixed = (seed.to_bits() ^ 0x9E37_79B9_7F4A_7C15).max(1);
    RANDOM_STATE.with(|state| state.set(mixed));
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn random_stays_in_range() {
        for _ in 0..100 {
            let value = random_unit();
            assert!((0.0..1.0).contains(&value), "{value}");
        }
    }

    #[test]
    fn absolutely_simple() {
        let mut interp = Interp::new();
        assert_eq!(
            interp.eval("return math.abs(-3)").unwrap(),
            Value::Number(3.0)
        );
        assert_eq!(
            interp.eval("return math.floor(1.7)").unwrap(),
            Value::Number(1.0)
        );
        assert_eq!(
            interp.eval("return math.max(1, 5, 3)").unwrap(),
            Value::Number(5.0)
        );
        assert_eq!(
            interp.eval("return math.min(1, 5, 3)").unwrap(),
            Value::Number(1.0)
        );
    }

    #[test]
    fn random_with_bounds_is_an_integer_in_range() {
        let mut interp = Interp::new();
        interp.eval("math.randomseed(1)").unwrap();
        for _ in 0..50 {
            let value = interp.eval("return math.random(6)").unwrap();
            let Value::Number(number) = value else {
                panic!("expected a number")
            };
            assert!((1.0..=6.0).contains(&number), "{number}");
        }
    }

    #[test]
    fn frexp_and_ldexp_round_trip() {
        let mut interp = Interp::new();
        let value = interp.eval("return math.ldexp(math.frexp(12.5))").unwrap();
        assert_eq!(value, Value::Number(12.5));
    }
}
