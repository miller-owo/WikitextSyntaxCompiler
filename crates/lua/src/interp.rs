use crate::ast::{
    BinaryOperator, Block, Chunk, Expression, FunctionBody, Statement, TableConstructor,
    UnaryOperator,
};
use crate::error::{Flow, LuaError, LuaResult};
use crate::lex::Span;
use crate::value::{EnvRef, LuaFunction, TableRef, Value};
use std::cell::RefCell;
use std::rc::Rc;
enum Target {
    Local(String, EnvRef),

    Global(String),

    Index(Value, Value),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub call_depth: usize,
    pub loop_iterations: usize,
    pub steps: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            call_depth: 200,
            loop_iterations: 1_000_000,
            steps: 20_000_000,
        }
    }
}

pub struct Interp {
    globals: EnvRef,
    chunk_name: RefCell<String>,
    current_line: RefCell<u32>,
    limits: Limits,
    depth: usize,
    steps: usize,
    string_metatable: RefCell<Option<TableRef>>,
}

impl Interp {
    pub fn new() -> Self {
        let mut interp = Interp {
            globals: EnvRef::root(),
            chunk_name: RefCell::new("?".to_string()),
            current_line: RefCell::new(0),
            limits: Limits::default(),
            depth: 0,
            steps: 0,
            string_metatable: RefCell::new(None),
        };
        crate::stdlib::install(&mut interp);
        interp
    }

    pub fn bare() -> Self {
        Interp {
            globals: EnvRef::root(),
            chunk_name: RefCell::new("?".to_string()),
            current_line: RefCell::new(0),
            limits: Limits::default(),
            depth: 0,
            steps: 0,
            string_metatable: RefCell::new(None),
        }
    }

    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    pub fn globals(&self) -> &EnvRef {
        &self.globals
    }

    pub fn global(&self, name: &str) -> Value {
        self.globals.get(name).unwrap_or(Value::Nil)
    }

    pub fn globals_table(&self) -> TableRef {
        let table = TableRef::new();
        for name in [
            "print",
            "type",
            "tostring",
            "tonumber",
            "pairs",
            "ipairs",
            "next",
            "select",
            "assert",
            "error",
            "pcall",
            "setmetatable",
            "getmetatable",
            "rawget",
            "rawset",
            "rawequal",
            "unpack",
            "collectgarbage",
            "string",
            "table",
            "math",
            "os",
        ] {
            table.set(Value::from(name), self.global(name));
        }
        table
    }

    pub fn set_global(&mut self, name: impl Into<String>, value: Value) {
        self.globals.define(name, value);
    }

    pub fn string_metatable(&self) -> Option<TableRef> {
        self.string_metatable.borrow().clone()
    }

    pub fn set_string_metatable(&mut self, metatable: TableRef) {
        *self.string_metatable.borrow_mut() = Some(metatable);
    }

    pub fn chunk_name(&self) -> String {
        self.chunk_name.borrow().clone()
    }

    pub fn eval(&mut self, source: &str) -> LuaResult<Value> {
        let values = self.eval_multi(source)?;
        Ok(values.into_iter().next().unwrap_or(Value::Nil))
    }

    pub fn eval_multi(&mut self, source: &str) -> LuaResult<Vec<Value>> {
        let chunk = crate::parse::parse(source, &self.chunk_name.borrow())?;
        self.run_chunk(&chunk)
    }

    pub fn run_chunk(&mut self, chunk: &Chunk) -> LuaResult<Vec<Value>> {
        *self.chunk_name.borrow_mut() = chunk.name.clone();
        self.steps = 0;
        let environment = EnvRef::child(&self.globals);
        match self.exec_block(&chunk.body, &environment)? {
            Flow::Return(values) => Ok(values),
            Flow::Normal | Flow::Break => Ok(Vec::new()),
        }
    }

    pub fn raise(&self, value: Value) -> LuaError {
        LuaError::at(value, self.span())
    }

    pub fn raise_str(&self, message: impl Into<String>) -> LuaError {
        self.raise(Value::string(message.into()))
    }

    fn span(&self) -> Span {
        Span::new(*self.current_line.borrow(), 1)
    }

    fn tick(&mut self, span: Span) -> LuaResult<()> {
        self.steps += 1;
        *self.current_line.borrow_mut() = span.line;
        if self.steps > self.limits.steps {
            return Err(LuaError::LimitExceeded {
                limit: "step budget",
            });
        }
        Ok(())
    }

    pub fn exec_block(&mut self, block: &Block, environment: &EnvRef) -> LuaResult<Flow> {
        for statement in &block.statements {
            let flow = self.exec_statement(statement, environment)?;
            if !matches!(flow, Flow::Normal) {
                return Ok(flow);
            }
        }
        if let Some(expressions) = &block.ret {
            let values = self.eval_expression_list(expressions, environment)?;
            return Ok(Flow::Return(values));
        }
        Ok(Flow::Normal)
    }

    fn exec_statement(&mut self, statement: &Statement, environment: &EnvRef) -> LuaResult<Flow> {
        match statement {
            Statement::Assign {
                targets,
                values,
                span,
            } => {
                self.tick(*span)?;

                let evaluated = self.eval_expression_list(values, environment)?;
                let mut resolved: Vec<(Target, Value)> = Vec::with_capacity(targets.len());
                for (index, target) in targets.iter().enumerate() {
                    let value = evaluated.get(index).cloned().unwrap_or(Value::Nil);
                    resolved.push((self.resolve_target(target, environment)?, value));
                }
                for (target, value) in resolved {
                    self.assign(target, value)?;
                }
                Ok(Flow::Normal)
            }
            Statement::Call(expression) => {
                self.eval_expression(expression, environment)?;
                Ok(Flow::Normal)
            }
            Statement::Do(block) => {
                let scope = EnvRef::child(environment);
                self.exec_block(block, &scope)
            }
            Statement::While { condition, body } => {
                let mut iterations = 0usize;
                while self.eval_expression(condition, environment)?.is_truthy() {
                    iterations += 1;
                    if iterations > self.limits.loop_iterations {
                        return Err(LuaError::LimitExceeded {
                            limit: "loop iteration",
                        });
                    }
                    let scope = EnvRef::child(environment);
                    match self.exec_block(body, &scope)? {
                        Flow::Break => break,
                        Flow::Return(values) => return Ok(Flow::Return(values)),
                        Flow::Normal => {}
                    }
                }
                Ok(Flow::Normal)
            }
            Statement::Repeat { body, condition } => {
                let mut iterations = 0usize;
                loop {
                    iterations += 1;
                    if iterations > self.limits.loop_iterations {
                        return Err(LuaError::LimitExceeded {
                            limit: "loop iteration",
                        });
                    }

                    let scope = EnvRef::child(environment);
                    match self.exec_block(body, &scope)? {
                        Flow::Break => break,
                        Flow::Return(values) => return Ok(Flow::Return(values)),
                        Flow::Normal => {}
                    }
                    if self.eval_expression(condition, &scope)?.is_truthy() {
                        break;
                    }
                }
                Ok(Flow::Normal)
            }
            Statement::If {
                condition,
                then_block,
                else_ifs,
                else_block,
            } => {
                if self.eval_expression(condition, environment)?.is_truthy() {
                    let scope = EnvRef::child(environment);
                    return self.exec_block(then_block, &scope);
                }
                for (condition, block) in else_ifs {
                    if self.eval_expression(condition, environment)?.is_truthy() {
                        let scope = EnvRef::child(environment);
                        return self.exec_block(block, &scope);
                    }
                }
                if let Some(block) = else_block {
                    let scope = EnvRef::child(environment);
                    return self.exec_block(block, &scope);
                }
                Ok(Flow::Normal)
            }
            Statement::NumericFor {
                name,
                start,
                stop,
                step,
                body,
            } => {
                let start_value = self.eval_expression(start, environment)?;
                let start = self.to_number(&start_value)?;
                let stop_value = self.eval_expression(stop, environment)?;
                let stop = self.to_number(&stop_value)?;
                let step = match step {
                    Some(expression) => {
                        let value = self.eval_expression(expression, environment)?;
                        self.to_number(&value)?
                    }
                    None => 1.0,
                };
                if step == 0.0 {
                    return Err(self.raise_str("'for' step is zero"));
                }
                let mut index = start;
                let mut iterations = 0usize;
                while if step > 0.0 {
                    index <= stop
                } else {
                    index >= stop
                } {
                    iterations += 1;
                    if iterations > self.limits.loop_iterations {
                        return Err(LuaError::LimitExceeded {
                            limit: "loop iteration",
                        });
                    }
                    let scope = EnvRef::child(environment);
                    scope.define(name.clone(), Value::Number(index));
                    match self.exec_block(body, &scope)? {
                        Flow::Break => break,
                        Flow::Return(values) => return Ok(Flow::Return(values)),
                        Flow::Normal => {}
                    }
                    index += step;
                }
                Ok(Flow::Normal)
            }
            Statement::GenericFor {
                names,
                expressions,
                body,
            } => {
                let mut iterator = self
                    .eval_expression_list(expressions, environment)?
                    .into_iter();
                let function = iterator.next().unwrap_or(Value::Nil);
                let state = iterator.next().unwrap_or(Value::Nil);
                let mut control = iterator.next().unwrap_or(Value::Nil);
                let mut iterations = 0usize;
                loop {
                    iterations += 1;
                    if iterations > self.limits.loop_iterations {
                        return Err(LuaError::LimitExceeded {
                            limit: "loop iteration",
                        });
                    }
                    let results =
                        self.call_value(function.clone(), vec![state.clone(), control.clone()])?;
                    let first = results.first().cloned().unwrap_or(Value::Nil);
                    if matches!(first, Value::Nil) {
                        break;
                    }
                    control = first;
                    let scope = EnvRef::child(environment);
                    for (index, name) in names.iter().enumerate() {
                        scope.define(
                            name.clone(),
                            results.get(index).cloned().unwrap_or(Value::Nil),
                        );
                    }
                    match self.exec_block(body, &scope)? {
                        Flow::Break => break,
                        Flow::Return(values) => return Ok(Flow::Return(values)),
                        Flow::Normal => {}
                    }
                }
                Ok(Flow::Normal)
            }
            Statement::FunctionDeclaration {
                path,
                function,
                span,
                method: _,
            } => {
                self.tick(*span)?;
                let closure = Value::Function(LuaFunction::closure(
                    (*function).clone(),
                    environment.clone(),
                ));
                self.name_function(&closure, path.last());
                if path.len() == 1 {
                    self.set_global(path[0].clone(), closure);
                    return Ok(Flow::Normal);
                }

                let mut base = self.eval_name(&path[0], environment)?;
                for part in &path[1..path.len() - 1] {
                    base = self.index(&base, &Value::string(part), *span)?;
                }
                self.set_index(&base, &Value::string(&path[path.len() - 1]), closure, *span)?;
                Ok(Flow::Normal)
            }
            Statement::LocalAssign {
                names,
                values,
                span,
            } => {
                self.tick(*span)?;
                let evaluated = self.eval_expression_list(values, environment)?;
                for (index, name) in names.iter().enumerate() {
                    environment.define(
                        name.clone(),
                        evaluated.get(index).cloned().unwrap_or(Value::Nil),
                    );
                }
                Ok(Flow::Normal)
            }
            Statement::LocalFunction {
                name,
                function,
                span,
            } => {
                self.tick(*span)?;

                environment.define(name.clone(), Value::Nil);
                let closure = Value::Function(LuaFunction::closure(
                    (*function).clone(),
                    environment.clone(),
                ));
                self.name_function(&closure, Some(name));
                environment.set(name, closure);
                Ok(Flow::Normal)
            }
            Statement::Break(span) => {
                self.tick(*span)?;
                Ok(Flow::Break)
            }
        }
    }

    fn name_function(&self, value: &Value, name: Option<&String>) {
        if let (Value::Function(function), Some(name)) = (value, name) {
            function.name.set(Some(name.clone()));
        }
    }

    fn resolve_target(&mut self, target: &Expression, environment: &EnvRef) -> LuaResult<Target> {
        match target {
            Expression::Name(name, _) => {
                if environment.get(name).is_some() {
                    Ok(Target::Local(name.clone(), environment.clone()))
                } else {
                    Ok(Target::Global(name.clone()))
                }
            }
            Expression::Index { base, key, span } => {
                let base = self.eval_expression(base, environment)?;
                let key = self.eval_expression(key, environment)?;
                let _ = span;
                Ok(Target::Index(base, key))
            }
            other => Err(self.raise_str(format!("cannot assign to {}", Self::describe(other)))),
        }
    }

    fn assign(&mut self, target: Target, value: Value) -> LuaResult<()> {
        match target {
            Target::Local(name, environment) => {
                environment.set(&name, value);
                Ok(())
            }
            Target::Global(name) => {
                self.globals.define(name, value);
                Ok(())
            }
            Target::Index(base, key) => self.set_index(&base, &key, value, self.span()),
        }
    }

    pub fn eval_expression(
        &mut self,
        expression: &Expression,
        environment: &EnvRef,
    ) -> LuaResult<Value> {
        let values = self.eval_expression_multi(expression, environment)?;
        Ok(values.into_iter().next().unwrap_or(Value::Nil))
    }

    pub fn eval_expression_multi(
        &mut self,
        expression: &Expression,
        environment: &EnvRef,
    ) -> LuaResult<Vec<Value>> {
        self.tick(expression.span())?;
        match expression {
            Expression::Nil(_) => Ok(vec![Value::Nil]),
            Expression::Boolean(flag, _) => Ok(vec![Value::Boolean(*flag)]),
            Expression::Number(value, _) => Ok(vec![Value::Number(*value)]),
            Expression::String(text, _) => Ok(vec![Value::string(text)]),
            Expression::Varargs(_) => Ok(environment.varargs()),
            Expression::Name(name, span) => self
                .eval_name(name, environment)
                .map(|value| vec![value])
                .map_err(|error| error.with_span(*span)),
            Expression::Function(body) => Ok(vec![Value::Function(LuaFunction::closure(
                (**body).clone(),
                environment.clone(),
            ))]),
            Expression::Table(constructor, span) => {
                Ok(vec![self.eval_table(constructor, environment, *span)?])
            }
            Expression::Index { base, key, span } => {
                let base = self.eval_expression(base, environment)?;
                let key = self.eval_expression(key, environment)?;
                self.index(&base, &key, *span).map(|value| vec![value])
            }
            Expression::Call {
                base,
                arguments,
                method,
                span,
            } => self.eval_call(base, method.as_deref(), arguments, environment, *span),
            Expression::Unary {
                operator,
                operand,
                span,
            } => {
                let value = self.eval_expression(operand, environment)?;
                self.eval_unary(*operator, value, *span)
                    .map(|value| vec![value])
            }
            Expression::Binary {
                operator,
                left,
                right,
                span,
            } => {
                match operator {
                    BinaryOperator::And => {
                        let left = self.eval_expression(left, environment)?;
                        if !left.is_truthy() {
                            return Ok(vec![left]);
                        }
                        return self.eval_expression_multi(right, environment);
                    }
                    BinaryOperator::Or => {
                        let left = self.eval_expression(left, environment)?;
                        if left.is_truthy() {
                            return Ok(vec![left]);
                        }
                        return self.eval_expression_multi(right, environment);
                    }
                    _ => {}
                }
                let left = self.eval_expression(left, environment)?;
                let right = self.eval_expression(right, environment)?;
                self.eval_binary(*operator, left, right, *span)
                    .map(|value| vec![value])
            }
        }
    }

    pub fn eval_expression_list(
        &mut self,
        expressions: &[Expression],
        environment: &EnvRef,
    ) -> LuaResult<Vec<Value>> {
        let mut values = Vec::with_capacity(expressions.len());
        for (index, expression) in expressions.iter().enumerate() {
            let is_last = index + 1 == expressions.len();
            if is_last {
                values.extend(self.eval_expression_multi(expression, environment)?);
            } else {
                values.push(self.eval_expression(expression, environment)?);
            }
        }
        Ok(values)
    }

    pub fn eval_expression_list_adjusted(
        &mut self,
        expressions: &[Expression],
        environment: &EnvRef,
        count: usize,
    ) -> LuaResult<Vec<Value>> {
        let mut values = self.eval_expression_list(expressions, environment)?;
        values.truncate(count);
        while values.len() < count {
            values.push(Value::Nil);
        }
        Ok(values)
    }

    fn eval_name(&mut self, name: &str, environment: &EnvRef) -> LuaResult<Value> {
        if let Some(value) = environment.get(name) {
            return Ok(value);
        }
        Ok(self.globals.get(name).unwrap_or(Value::Nil))
    }

    fn eval_table(
        &mut self,
        constructor: &TableConstructor,
        environment: &EnvRef,
        span: Span,
    ) -> LuaResult<Value> {
        let _ = span;
        let table = TableRef::new();

        let mut index = 1usize;
        for (position, item) in constructor.items.iter().enumerate() {
            let is_last = position + 1 == constructor.items.len() && constructor.fields.is_empty();
            if is_last {
                for value in self.eval_expression_multi(item, environment)? {
                    table.set(Value::from(index), value);
                    index += 1;
                }
            } else {
                let value = self.eval_expression(item, environment)?;
                table.set(Value::from(index), value);
                index += 1;
            }
        }
        for (key, value) in &constructor.fields {
            let key = self.eval_expression(key, environment)?;
            let value = self.eval_expression(value, environment)?;
            table.set(key, value);
        }
        Ok(Value::Table(table))
    }

    fn eval_call(
        &mut self,
        base: &Expression,
        method: Option<&str>,
        arguments: &[Expression],
        environment: &EnvRef,
        span: Span,
    ) -> LuaResult<Vec<Value>> {
        let receiver = self.eval_expression(base, environment)?;
        let (callee, mut values) = match method {
            Some(name) => {
                let function = self.index(&receiver, &Value::string(name), span)?;
                (function, vec![receiver])
            }
            None => (receiver, Vec::new()),
        };
        values.extend(self.eval_expression_list(arguments, environment)?);
        self.call_value(callee, values)
            .map_err(|error| error.with_span(span))
    }

    pub fn call_value(&mut self, callee: Value, arguments: Vec<Value>) -> LuaResult<Vec<Value>> {
        match callee {
            Value::Function(function) => self.call_function(&function, arguments),
            Value::Table(table) => {
                let metamethod = self.metamethod_table(&table, "__call");
                match metamethod {
                    Some(Value::Function(function)) => {
                        let mut values = vec![Value::Table(table)];
                        values.extend(arguments);
                        self.call_function(&function, values)
                    }
                    Some(other) => {
                        Err(self
                            .raise_str(format!("attempt to call a {} value", other.type_name())))
                    }
                    None => Err(self.raise_str("attempt to call a table value")),
                }
            }
            other => Err(self.raise_str(format!("attempt to call a {} value", other.type_name()))),
        }
    }

    pub fn call_function(
        &mut self,
        function: &Rc<LuaFunction>,
        arguments: Vec<Value>,
    ) -> LuaResult<Vec<Value>> {
        if self.depth >= self.limits.call_depth {
            return Err(LuaError::LimitExceeded {
                limit: "call depth",
            });
        }
        self.depth += 1;
        let result = match &function.kind {
            crate::value::FunctionKind::Native { implementation, .. } => {
                implementation(self, &arguments)
            }
            crate::value::FunctionKind::Closure { body, environment } => {
                self.call_closure(body, environment, arguments)
            }
        };
        self.depth -= 1;
        result
    }

    fn call_closure(
        &mut self,
        body: &FunctionBody,
        environment: &EnvRef,
        arguments: Vec<Value>,
    ) -> LuaResult<Vec<Value>> {
        let scope = EnvRef::child(environment);
        for (index, parameter) in body.parameters.iter().enumerate() {
            scope.define(
                parameter.clone(),
                arguments.get(index).cloned().unwrap_or(Value::Nil),
            );
        }
        if body.varargs {
            let extra = arguments
                .iter()
                .skip(body.parameters.len())
                .cloned()
                .collect();
            scope.set_varargs(extra);
        }
        match self.exec_block(&body.body, &scope)? {
            Flow::Return(values) => Ok(values),
            Flow::Normal | Flow::Break => Ok(Vec::new()),
        }
    }

    pub fn index(&mut self, base: &Value, key: &Value, span: Span) -> LuaResult<Value> {
        match base {
            Value::Table(table) => {
                let value = table.get(key);
                if !matches!(value, Value::Nil) {
                    return Ok(value);
                }
                match self.metamethod_table(table, "__index") {
                    Some(handler) => match handler {
                        Value::Function(function) => {
                            let results = self.call_function(
                                &function,
                                vec![Value::Table(table.clone()), key.clone()],
                            )?;
                            Ok(results.into_iter().next().unwrap_or(Value::Nil))
                        }
                        Value::Table(handler_table) => {
                            self.index(&Value::Table(handler_table), key, span)
                        }
                        _ => Ok(Value::Nil),
                    },
                    None => Ok(Value::Nil),
                }
            }
            Value::String(_) => {
                let handler = self
                    .string_metatable
                    .borrow()
                    .clone()
                    .map(|metatable| metatable.get(&Value::string("__index")));
                match handler {
                    Some(Value::Table(handler)) => {
                        let value = handler.get(key);
                        if !matches!(value, Value::Nil) {
                            return Ok(value);
                        }
                    }
                    Some(Value::Function(function)) => {
                        let results =
                            self.call_function(&function, vec![base.clone(), key.clone()])?;
                        return Ok(results.into_iter().next().unwrap_or(Value::Nil));
                    }
                    _ => {}
                }
                Err(self
                    .raise_str("attempt to index a string value")
                    .with_span(span))
            }
            other => Err(self
                .raise_str(format!("attempt to index a {} value", other.type_name()))
                .with_span(span)),
        }
    }

    pub fn set_index(
        &mut self,
        base: &Value,
        key: &Value,
        value: Value,
        span: Span,
    ) -> LuaResult<()> {
        match base {
            Value::Table(table) => {
                let existing = table.get(key);
                if !matches!(existing, Value::Nil) {
                    table.set(key.clone(), value);
                    return Ok(());
                }
                match self.metamethod_table(table, "__newindex") {
                    Some(handler) => match handler {
                        Value::Function(function) => {
                            self.call_function(
                                &function,
                                vec![Value::Table(table.clone()), key.clone(), value],
                            )?;
                            Ok(())
                        }
                        Value::Table(handler_table) => {
                            self.set_index(&Value::Table(handler_table), key, value, span)
                        }
                        _ => {
                            table.set(key.clone(), value);
                            Ok(())
                        }
                    },
                    None => {
                        table.set(key.clone(), value);
                        Ok(())
                    }
                }
            }
            other => Err(self
                .raise_str(format!("attempt to index a {} value", other.type_name()))
                .with_span(span)),
        }
    }

    fn metamethod_table(&self, table: &TableRef, name: &str) -> Option<Value> {
        let metatable = table.metatable()?;
        let value = metatable.get(&Value::string(name));
        if matches!(value, Value::Nil) {
            None
        } else {
            Some(value)
        }
    }

    pub fn metamethod(&self, value: &Value, name: &str) -> Option<Value> {
        let metatable = match value {
            Value::Table(table) => table.metatable(),
            Value::String(_) => self.string_metatable.borrow().clone(),
            Value::UserData(data) => data.metatable.clone(),
            _ => None,
        }?;
        let found = metatable.get(&Value::string(name));
        if matches!(found, Value::Nil) {
            None
        } else {
            Some(found)
        }
    }

    pub fn metatable_of(&self, value: &Value) -> Option<TableRef> {
        match value {
            Value::Table(table) => table.metatable(),
            Value::String(_) => self.string_metatable.borrow().clone(),
            Value::UserData(data) => data.metatable.clone(),
            _ => None,
        }
    }

    fn eval_unary(
        &mut self,
        operator: UnaryOperator,
        value: Value,
        span: Span,
    ) -> LuaResult<Value> {
        match operator {
            UnaryOperator::Not => Ok(Value::Boolean(!value.is_truthy())),
            UnaryOperator::Negate => match value {
                Value::Number(number) => Ok(Value::Number(-number)),
                other => {
                    if let Some(handler) = self.metamethod(&other, "__unm") {
                        let results = self.call_value(handler, vec![other])?;
                        return Ok(results.into_iter().next().unwrap_or(Value::Nil));
                    }
                    Err(self
                        .raise_str(format!(
                            "attempt to perform arithmetic on a {} value",
                            other.type_name()
                        ))
                        .with_span(span))
                }
            },
            UnaryOperator::Length => match &value {
                Value::String(text) => Ok(Value::Number(text.chars().count() as f64)),
                Value::Table(table) => {
                    if let Some(handler) = self.metamethod(&value, "__len") {
                        let results = self.call_value(handler, vec![value.clone()])?;
                        return Ok(results.into_iter().next().unwrap_or(Value::Nil));
                    }
                    Ok(Value::Number(table.length() as f64))
                }
                other => {
                    if let Some(handler) = self.metamethod(other, "__len") {
                        let results = self.call_value(handler, vec![value.clone()])?;
                        return Ok(results.into_iter().next().unwrap_or(Value::Nil));
                    }
                    Err(self
                        .raise_str(format!(
                            "attempt to get length of a {} value",
                            other.type_name()
                        ))
                        .with_span(span))
                }
            },
        }
    }

    fn eval_binary(
        &mut self,
        operator: BinaryOperator,
        left: Value,
        right: Value,
        span: Span,
    ) -> LuaResult<Value> {
        match operator {
            BinaryOperator::Equal => return self.eval_equal(&left, &right),
            BinaryOperator::NotEqual => {
                return Ok(Value::Boolean(!self.eval_equal(&left, &right)?.is_truthy()))
            }
            BinaryOperator::Less
            | BinaryOperator::Greater
            | BinaryOperator::LessEqual
            | BinaryOperator::GreaterEqual => {
                return self.eval_comparison(operator, left, right, span)
            }
            BinaryOperator::Concat => return self.eval_concat(left, right, span),
            _ => {}
        }

        let (left_number, right_number) = match (left.as_number(), right.as_number()) {
            (Some(a), Some(b)) => (a, b),
            _ => {
                let name = match operator {
                    BinaryOperator::Add => "__add",
                    BinaryOperator::Subtract => "__sub",
                    BinaryOperator::Multiply => "__mul",
                    BinaryOperator::Divide => "__div",
                    BinaryOperator::Modulo => "__mod",
                    BinaryOperator::Power => "__pow",
                    _ => "__add",
                };
                let handler = self
                    .metamethod(&left, name)
                    .or_else(|| self.metamethod(&right, name));
                if let Some(handler) = handler {
                    let results = self.call_value(handler, vec![left, right])?;
                    return Ok(results.into_iter().next().unwrap_or(Value::Nil));
                }
                let offender = if left.as_number().is_none() {
                    &left
                } else {
                    &right
                };
                return Err(self
                    .raise_str(format!(
                        "attempt to perform arithmetic on a {} value",
                        offender.type_name()
                    ))
                    .with_span(span));
            }
        };
        let result = match operator {
            BinaryOperator::Add => left_number + right_number,
            BinaryOperator::Subtract => left_number - right_number,
            BinaryOperator::Multiply => left_number * right_number,
            BinaryOperator::Divide => left_number / right_number,
            BinaryOperator::Modulo => {
                left_number - (left_number / right_number).floor() * right_number
            }
            BinaryOperator::Power => left_number.powf(right_number),
            _ => unreachable!("arith operator"),
        };
        Ok(Value::Number(result))
    }

    fn eval_equal(&mut self, left: &Value, right: &Value) -> LuaResult<Value> {
        let equal = match (left, right) {
            (Value::Number(a), Value::Number(b)) => *a == *b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            _ => {
                let handler = self
                    .metamethod(left, "__eq")
                    .or_else(|| self.metamethod(right, "__eq"));
                match handler {
                    Some(handler) => {
                        let results =
                            self.call_value(handler, vec![left.clone(), right.clone()])?;
                        return Ok(Value::Boolean(
                            results.first().is_some_and(Value::is_truthy),
                        ));
                    }
                    None => left == right,
                }
            }
        };
        Ok(Value::Boolean(equal))
    }

    fn eval_comparison(
        &mut self,
        operator: BinaryOperator,
        left: Value,
        right: Value,
        span: Span,
    ) -> LuaResult<Value> {
        let (a, b) = match (left.as_number(), right.as_number()) {
            (Some(a), Some(b)) => (a, b),
            _ => match (&left, &right) {
                (Value::String(a), Value::String(b)) => {
                    let ordering = a.as_ref().cmp(b.as_ref());
                    let result = match operator {
                        BinaryOperator::Less => ordering.is_lt(),
                        BinaryOperator::LessEqual => ordering.is_le(),
                        BinaryOperator::Greater => ordering.is_gt(),
                        _ => ordering.is_ge(),
                    };
                    return Ok(Value::Boolean(result));
                }
                _ => {
                    let (name, swap) = match operator {
                        BinaryOperator::Less => ("__lt", false),
                        BinaryOperator::LessEqual => ("__le", false),
                        BinaryOperator::Greater => ("__lt", true),
                        _ => ("__le", true),
                    };
                    let handler = if swap {
                        self.metamethod(&right, name)
                            .or_else(|| self.metamethod(&left, name))
                    } else {
                        self.metamethod(&left, name)
                            .or_else(|| self.metamethod(&right, name))
                    };
                    match handler {
                        Some(handler) => {
                            let (first, second) = if swap {
                                (right.clone(), left.clone())
                            } else {
                                (left.clone(), right.clone())
                            };
                            let results = self.call_value(handler, vec![first, second])?;
                            return Ok(Value::Boolean(
                                results.first().is_some_and(Value::is_truthy),
                            ));
                        }
                        None => {
                            return Err(self
                                .raise_str(format!(
                                    "attempt to compare {} with {}",
                                    left.type_name(),
                                    right.type_name()
                                ))
                                .with_span(span))
                        }
                    }
                }
            },
        };
        let result = match operator {
            BinaryOperator::Less => a < b,
            BinaryOperator::LessEqual => a <= b,
            BinaryOperator::Greater => a > b,
            _ => a >= b,
        };
        Ok(Value::Boolean(result))
    }

    fn eval_concat(&mut self, left: Value, right: Value, span: Span) -> LuaResult<Value> {
        let left_text = self.coerce_concat(&left);
        let right_text = self.coerce_concat(&right);
        match (left_text, right_text) {
            (Some(a), Some(b)) => Ok(Value::string(format!("{a}{b}"))),
            _ => {
                let handler = self
                    .metamethod(&left, "__concat")
                    .or_else(|| self.metamethod(&right, "__concat"));
                match handler {
                    Some(handler) => {
                        let results = self.call_value(handler, vec![left, right])?;
                        Ok(results.into_iter().next().unwrap_or(Value::Nil))
                    }
                    None => Err(self
                        .raise_str(format!(
                            "attempt to concatenate a {} value",
                            if self.coerce_concat(&left).is_none() {
                                left.type_name()
                            } else {
                                right.type_name()
                            }
                        ))
                        .with_span(span)),
                }
            }
        }
    }

    fn coerce_concat(&self, value: &Value) -> Option<String> {
        match value {
            Value::String(text) => Some(text.to_string()),
            Value::Number(number) => Some(Value::format_number(*number)),
            _ => None,
        }
    }

    pub fn to_number(&self, value: &Value) -> LuaResult<f64> {
        value.as_number().ok_or_else(|| {
            self.raise_str(format!(
                "attempt to perform arithmetic on a {} value",
                value.type_name()
            ))
        })
    }

    pub fn to_integer(&self, value: &Value) -> LuaResult<i64> {
        let number = self.to_number(value)?;
        if number.fract() != 0.0 {
            return Err(self.raise_str(format!("number has no integer representation: {number}")));
        }
        Ok(number as i64)
    }

    pub fn to_string_value(&self, value: &Value) -> Option<String> {
        match value {
            Value::String(text) => Some(text.to_string()),
            Value::Number(number) => Some(Value::format_number(*number)),
            _ => None,
        }
    }

    pub fn tostring(&mut self, value: &Value) -> LuaResult<String> {
        if let Some(handler) = self.metamethod(value, "__tostring") {
            let results = self.call_value(handler, vec![value.clone()])?;
            return match results.first() {
                Some(Value::String(text)) => Ok(text.to_string()),
                Some(other) => Ok(other.to_string()),
                None => Ok(value.to_string()),
            };
        }
        Ok(match value {
            Value::String(text) => text.to_string(),
            Value::Number(number) => Value::format_number(*number),
            other => other.to_string(),
        })
    }

    fn describe(expression: &Expression) -> String {
        match expression {
            Expression::Name(name, _) => format!("`{name}`"),
            Expression::Call { .. } => "a function call".to_string(),
            Expression::Binary { .. } => "an expression".to_string(),
            _ => "this expression".to_string(),
        }
    }
}

impl Default for Interp {
    fn default() -> Self {
        Interp::new()
    }
}

impl core::fmt::Debug for Interp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Interp")
            .field("globals", &self.globals)
            .field("depth", &self.depth)
            .finish_non_exhaustive()
    }
}
