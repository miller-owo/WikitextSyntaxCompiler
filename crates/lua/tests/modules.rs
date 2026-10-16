use wikitext_lua::value::TableRef;
use wikitext_lua::{Interp, Value};
fn load(source: &str) -> (Interp, TableRef) {
    let mut interp = Interp::new();
    let values = interp
        .eval_multi(source)
        .unwrap_or_else(|error| panic!("module failed to load: {error}"));
    let exports = match values.first() {
        Some(Value::Table(table)) => table.clone(),
        other => panic!("a module must return a table, got {other:?}"),
    };
    (interp, exports)
}

fn call(interp: &mut Interp, exports: &TableRef, name: &str, args: &[&str]) -> String {
    let function = exports.get(&Value::string(name));
    assert!(
        matches!(function, Value::Function(_)),
        "`{name}` is not a function in this module"
    );
    let arguments = TableRef::new();
    for (index, argument) in args.iter().enumerate() {
        arguments.set(Value::from(index + 1), Value::string(*argument));
    }
    let frame = TableRef::new();
    frame.set(Value::from("args"), Value::Table(arguments));
    let results = interp
        .call_value(function, vec![Value::Table(frame)])
        .unwrap_or_else(|error| panic!("{name} raised: {error}"));
    results
        .first()
        .map(|value| interp.tostring(value).unwrap_or_default())
        .unwrap_or_default()
}

const MODULE: &str = r#"
local p = {}

local function trim(text)
    return (text:gsub("^%s*(.-)%s*$", "%1"))
end

local function split(text, separator)
    local parts = {}
    for piece in text:gmatch("[^" .. separator .. "]+") do
        parts[#parts + 1] = trim(piece)
    end
    return parts
end

function p.greet(frame)
    return "Hello, " .. (frame.args[1] or "world") .. "!"
end

function p.count(frame)
    return #split(frame.args[1] or "", " ")
end

function p.upper(frame)
    return (frame.args[1] or ""):upper()
end

function p.repeat_row(frame)
    local rows = {}
    for i = 1, tonumber(frame.args[1] or "3") do
        rows[#rows + 1] = string.format("| %d || %s", i, ("x"):rep(i))
    end
    return table.concat(rows, "\n")
end

function p.descending(frame)
    local numbers = {3, 1, 4, 1, 5, 9, 2, 6}
    table.sort(numbers, function(a, b) return a > b end)
    return table.concat(numbers, ",")
end

function p.caught(frame)
    local ok, message = pcall(function() error("boom") end)
    if ok then return "no error" end
    return "caught: " .. tostring(message)
end

function p.metatable(frame)
    local point = {x = 1, y = 2}
    setmetatable(point, {
        __index = function(_, key) return "missing:" .. key end,
        __tostring = function(self) return "(" .. self.x .. "," .. self.y .. ")" end,
    })
    return tostring(point) .. " " .. point.z
end

function p.counter(frame)
    local function make()
        local n = 0
        return function()
            n = n + 1
            return n
        end
    end
    local next_number = make()
    next_number()
    next_number()
    return next_number()
end

function p.varargs(frame)
    local function total(...)
        local sum = 0
        for _, value in ipairs({...}) do
            sum = sum + value
        end
        return sum
    end
    return total(1, 2, 3, 4)
end

function p.roman(frame)
    local numerals = {
        {1000, "M"}, {900, "CM"}, {500, "D"}, {400, "CD"},
        {100, "C"}, {90, "XC"}, {50, "L"}, {40, "XL"},
        {10, "X"}, {9, "IX"}, {5, "V"}, {4, "IV"}, {1, "I"},
    }
    local value = tonumber(frame.args[1] or "0")
    local out = {}
    for _, pair in ipairs(numerals) do
        while value >= pair[1] do
            out[#out + 1] = pair[2]
            value = value - pair[1]
        end
    end
    return table.concat(out)
end

return p
"#;

#[test]
fn module_loads_and_exports_a_table() {
    let (_, exports) = load(MODULE);
    assert!(exports.len() >= 10, "expected the exported functions");
}

#[test]
fn string_arguments_reach_the_module() {
    let (mut interp, exports) = load(MODULE);
    assert_eq!(
        call(&mut interp, &exports, "greet", &["Rust"]),
        "Hello, Rust!"
    );
    assert_eq!(call(&mut interp, &exports, "greet", &[]), "Hello, world!");
    assert_eq!(call(&mut interp, &exports, "upper", &["abc"]), "ABC");
}

#[test]
fn patterns_split_text() {
    let (mut interp, exports) = load(MODULE);
    assert_eq!(
        call(&mut interp, &exports, "count", &["one two three"]),
        "3"
    );

    assert_eq!(call(&mut interp, &exports, "count", &["a  b   c"]), "3");
}

#[test]
fn loops_and_string_format_build_wikitext() {
    let (mut interp, exports) = load(MODULE);
    let output = call(&mut interp, &exports, "repeat_row", &["2"]);
    assert_eq!(output, "| 1 || x\n| 2 || xx");
}

#[test]
fn sorting_with_a_comparator_works() {
    let (mut interp, exports) = load(MODULE);
    assert_eq!(
        call(&mut interp, &exports, "descending", &[]),
        "9,6,5,4,3,2,1,1"
    );
}

#[test]
fn errors_are_catchable() {
    let (mut interp, exports) = load(MODULE);
    assert_eq!(call(&mut interp, &exports, "caught", &[]), "caught: boom");
}

#[test]
fn metatables_drive_indexing_and_tostring() {
    let (mut interp, exports) = load(MODULE);
    assert_eq!(
        call(&mut interp, &exports, "metatable", &[]),
        "(1,2) missing:z"
    );
}

#[test]
fn closures_share_their_upvalues() {
    let (mut interp, exports) = load(MODULE);
    assert_eq!(call(&mut interp, &exports, "counter", &[]), "3");
}

#[test]
fn varargs_are_collected() {
    let (mut interp, exports) = load(MODULE);
    assert_eq!(call(&mut interp, &exports, "varargs", &[]), "10");
}

#[test]
fn a_realistic_module_computes_correctly() {
    let (mut interp, exports) = load(MODULE);
    assert_eq!(call(&mut interp, &exports, "roman", &["1994"]), "MCMXCIV");
    assert_eq!(call(&mut interp, &exports, "roman", &["4"]), "IV");
}

#[test]
fn a_runtime_error_reports_a_position() {
    let mut interp = Interp::new();
    let error = interp.eval("local x = nil\nreturn x.field").unwrap_err();
    let rendered = error.to_string();
    assert!(rendered.contains("attempt to index"), "{rendered}");
    assert!(error.span().is_some(), "the error should carry a line");
}

#[test]
fn recursion_is_bounded() {
    let mut interp = Interp::new();
    let error = interp
        .eval("local function loop() return loop() end return loop()")
        .unwrap_err();
    assert!(
        matches!(error, wikitext_lua::LuaError::LimitExceeded { .. }),
        "{error}"
    );
}

#[test]
fn infinite_loops_are_bounded() {
    let mut interp = Interp::new();
    let error = interp.eval("while true do end").unwrap_err();
    assert!(
        matches!(error, wikitext_lua::LuaError::LimitExceeded { .. }),
        "{error}"
    );
}

#[test]
fn syntax_errors_are_reported_before_running() {
    let mut interp = Interp::new();
    let error = interp.eval("if x then").unwrap_err();
    assert!(error.to_string().contains("end"), "{error}");
}

#[test]
fn pcall_does_not_swallow_the_step_budget() {
    let mut interp = Interp::new();
    let error = interp
        .eval("pcall(function() while true do end end)")
        .unwrap_err();
    assert!(
        matches!(error, wikitext_lua::LuaError::LimitExceeded { .. }),
        "a budget error must escape pcall: {error}"
    );
}
