use crate::ast::{
    BinaryOperator, Block, Chunk, Expression, FunctionBody, Statement, TableConstructor,
    UnaryOperator,
};
use crate::lex::{Keyword, LexError, Span, Spanned, Token};
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.span, self.message)
    }
}

impl std::error::Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(error: LexError) -> Self {
        ParseError {
            message: error.message,
            span: error.span,
        }
    }
}

pub fn parse(source: &str, name: &str) -> Result<Chunk, ParseError> {
    let tokens = crate::lex::tokenize(source)?;
    let mut parser = Parser { tokens, index: 0 };
    let body = parser.parse_block()?;
    parser.expect_eof()?;
    Ok(Chunk {
        body,
        name: name.to_string(),
    })
}

struct Parser {
    tokens: Vec<Spanned>,
    index: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.index].token
    }

    fn peek_at(&self, ahead: usize) -> &Token {
        let index = (self.index + ahead).min(self.tokens.len() - 1);
        &self.tokens[index].token
    }

    fn span(&self) -> Span {
        self.tokens[self.index].span
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.index].token.clone();
        if self.index + 1 < self.tokens.len() {
            self.index += 1;
        }
        token
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        ParseError {
            message: message.into(),
            span: self.span(),
        }
    }

    fn check_symbol(&self, symbol: &str) -> bool {
        matches!(self.peek(), Token::Symbol(found) if *found == symbol)
    }

    fn check_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.peek(), Token::Keyword(found) if *found == keyword)
    }

    fn accept_symbol(&mut self, symbol: &str) -> bool {
        if self.check_symbol(symbol) {
            self.advance();
            return true;
        }
        false
    }

    fn accept_keyword(&mut self, keyword: Keyword) -> bool {
        if self.check_keyword(keyword) {
            self.advance();
            return true;
        }
        false
    }

    fn expect_symbol(&mut self, symbol: &str) -> Result<(), ParseError> {
        if self.accept_symbol(symbol) {
            return Ok(());
        }
        Err(self.error(format!(
            "expected `{symbol}`, found {}",
            self.peek().describe()
        )))
    }

    fn expect_keyword(&mut self, keyword: Keyword) -> Result<(), ParseError> {
        if self.accept_keyword(keyword) {
            return Ok(());
        }
        Err(self.error(format!(
            "expected `{}`, found {}",
            keyword.as_str(),
            self.peek().describe()
        )))
    }

    fn expect_name(&mut self) -> Result<String, ParseError> {
        match self.peek().clone() {
            Token::Name(name) => {
                self.advance();
                Ok(name)
            }
            other => Err(self.error(format!("expected a name, found {}", other.describe()))),
        }
    }

    fn expect_eof(&mut self) -> Result<(), ParseError> {
        if matches!(self.peek(), Token::Eof) {
            return Ok(());
        }
        Err(self.error(format!("unexpected {}", self.peek().describe())))
    }

    fn at_block_end(&self) -> bool {
        matches!(self.peek(), Token::Eof)
            || self.check_keyword(Keyword::End)
            || self.check_keyword(Keyword::Else)
            || self.check_keyword(Keyword::Elseif)
            || self.check_keyword(Keyword::Until)
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let mut block = Block::default();
        while !self.at_block_end() {
            if self.check_keyword(Keyword::Return) {
                self.advance();

                block.ret = Some(if self.at_block_end() || self.check_symbol(";") {
                    Vec::new()
                } else {
                    self.parse_expression_list()?
                });

                self.accept_symbol(";");
                break;
            }
            if let Some(statement) = self.parse_statement()? {
                block.statements.push(statement);
            }
        }
        Ok(block)
    }

    fn parse_statement(&mut self) -> Result<Option<Statement>, ParseError> {
        let span = self.span();
        if self.accept_symbol(";") {
            return Ok(None);
        }
        if self.check_keyword(Keyword::If) {
            return self.parse_if().map(Some);
        }
        if self.check_keyword(Keyword::While) {
            return self.parse_while().map(Some);
        }
        if self.check_keyword(Keyword::Do) {
            self.advance();
            let body = self.parse_block()?;
            self.expect_keyword(Keyword::End)?;
            return Ok(Some(Statement::Do(body)));
        }
        if self.check_keyword(Keyword::For) {
            return self.parse_for().map(Some);
        }
        if self.check_keyword(Keyword::Repeat) {
            return self.parse_repeat().map(Some);
        }
        if self.check_keyword(Keyword::Function) {
            return self.parse_function_declaration().map(Some);
        }
        if self.check_keyword(Keyword::Local) {
            return self.parse_local().map(Some);
        }
        if self.check_keyword(Keyword::Break) {
            self.advance();
            return Ok(Some(Statement::Break(span)));
        }
        self.parse_expression_statement().map(Some)
    }

    fn parse_if(&mut self) -> Result<Statement, ParseError> {
        self.expect_keyword(Keyword::If)?;
        let condition = self.parse_expression()?;
        self.expect_keyword(Keyword::Then)?;
        let then_block = self.parse_block()?;
        let mut else_ifs = Vec::new();
        while self.check_keyword(Keyword::Elseif) {
            self.advance();
            let condition = self.parse_expression()?;
            self.expect_keyword(Keyword::Then)?;
            let block = self.parse_block()?;
            else_ifs.push((condition, block));
        }
        let else_block = if self.accept_keyword(Keyword::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };
        self.expect_keyword(Keyword::End)?;
        Ok(Statement::If {
            condition,
            then_block,
            else_ifs,
            else_block,
        })
    }

    fn parse_while(&mut self) -> Result<Statement, ParseError> {
        self.expect_keyword(Keyword::While)?;
        let condition = self.parse_expression()?;
        self.expect_keyword(Keyword::Do)?;
        let body = self.parse_block()?;
        self.expect_keyword(Keyword::End)?;
        Ok(Statement::While { condition, body })
    }

    fn parse_repeat(&mut self) -> Result<Statement, ParseError> {
        self.expect_keyword(Keyword::Repeat)?;
        let body = self.parse_block()?;
        self.expect_keyword(Keyword::Until)?;
        let condition = self.parse_expression()?;
        Ok(Statement::Repeat { body, condition })
    }

    fn parse_for(&mut self) -> Result<Statement, ParseError> {
        self.expect_keyword(Keyword::For)?;
        let first = self.expect_name()?;
        if self.accept_symbol("=") {
            let start = self.parse_expression()?;
            self.expect_symbol(",")?;
            let stop = self.parse_expression()?;
            let step = if self.accept_symbol(",") {
                Some(self.parse_expression()?)
            } else {
                None
            };
            self.expect_keyword(Keyword::Do)?;
            let body = self.parse_block()?;
            self.expect_keyword(Keyword::End)?;
            return Ok(Statement::NumericFor {
                name: first,
                start,
                stop,
                step,
                body,
            });
        }
        let mut names = vec![first];
        while self.accept_symbol(",") {
            names.push(self.expect_name()?);
        }
        self.expect_keyword(Keyword::In)?;
        let expressions = self.parse_expression_list()?;
        self.expect_keyword(Keyword::Do)?;
        let body = self.parse_block()?;
        self.expect_keyword(Keyword::End)?;
        Ok(Statement::GenericFor {
            names,
            expressions,
            body,
        })
    }

    fn parse_function_declaration(&mut self) -> Result<Statement, ParseError> {
        let span = self.span();
        self.expect_keyword(Keyword::Function)?;
        let mut path = vec![self.expect_name()?];
        let mut method = false;
        loop {
            if self.accept_symbol(".") {
                path.push(self.expect_name()?);
                continue;
            }
            if self.accept_symbol(":") {
                path.push(self.expect_name()?);
                method = true;
            }
            break;
        }
        let function = self.parse_function_body(span, method)?;
        Ok(Statement::FunctionDeclaration {
            path,
            method,
            function,
            span,
        })
    }

    fn parse_local(&mut self) -> Result<Statement, ParseError> {
        let span = self.span();
        self.expect_keyword(Keyword::Local)?;
        if self.accept_keyword(Keyword::Function) {
            let name = self.expect_name()?;
            let function = self.parse_function_body(span, false)?;
            return Ok(Statement::LocalFunction {
                name,
                function,
                span,
            });
        }
        let mut names = vec![self.expect_name()?];
        while self.accept_symbol(",") {
            names.push(self.expect_name()?);
        }
        let values = if self.accept_symbol("=") {
            self.parse_expression_list()?
        } else {
            Vec::new()
        };
        Ok(Statement::LocalAssign {
            names,
            values,
            span,
        })
    }

    fn parse_function_body(
        &mut self,
        span: Span,
        method: bool,
    ) -> Result<FunctionBody, ParseError> {
        self.expect_symbol("(")?;
        let mut parameters = Vec::new();
        let mut varargs = false;
        if method {
            parameters.push("self".to_string());
        }
        if !self.check_symbol(")") {
            loop {
                if self.accept_symbol("...") {
                    varargs = true;
                    break;
                }
                parameters.push(self.expect_name()?);
                if !self.accept_symbol(",") {
                    break;
                }
            }
        }
        self.expect_symbol(")")?;
        let body = self.parse_block()?;
        self.expect_keyword(Keyword::End)?;
        Ok(FunctionBody {
            parameters,
            varargs,
            body,
            span,
        })
    }

    fn parse_expression_statement(&mut self) -> Result<Statement, ParseError> {
        let span = self.span();
        let first = self.parse_suffixed_expression()?;
        if self.check_symbol("=") || self.check_symbol(",") {
            let mut targets = vec![first];
            while self.accept_symbol(",") {
                targets.push(self.parse_suffixed_expression()?);
            }
            for target in &targets {
                if !target.is_assignable() {
                    return Err(ParseError {
                        message: "cannot assign to this expression".to_string(),
                        span: target.span(),
                    });
                }
            }
            self.expect_symbol("=")?;
            let values = self.parse_expression_list()?;
            return Ok(Statement::Assign {
                targets,
                values,
                span,
            });
        }
        if matches!(first, Expression::Call { .. }) {
            return Ok(Statement::Call(first));
        }
        Err(ParseError {
            message: "expected a statement".to_string(),
            span,
        })
    }

    fn parse_expression_list(&mut self) -> Result<Vec<Expression>, ParseError> {
        let mut expressions = vec![self.parse_expression()?];
        while self.accept_symbol(",") {
            expressions.push(self.parse_expression()?);
        }
        Ok(expressions)
    }

    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_binary(1)
    }

    fn parse_binary(&mut self, minimum: u8) -> Result<Expression, ParseError> {
        let mut left = self.parse_unary()?;
        while let Some(operator) = self.peek_binary_operator() {
            let precedence = operator.precedence();
            if precedence < minimum {
                break;
            }
            let span = left.span();
            self.advance();

            let next_minimum = if matches!(operator, BinaryOperator::Power | BinaryOperator::Concat)
            {
                precedence
            } else {
                precedence + 1
            };
            let right = self.parse_binary(next_minimum)?;
            left = Expression::Binary {
                operator,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn peek_binary_operator(&self) -> Option<BinaryOperator> {
        Some(match self.peek() {
            Token::Keyword(Keyword::Or) => BinaryOperator::Or,
            Token::Keyword(Keyword::And) => BinaryOperator::And,
            Token::Symbol("<") => BinaryOperator::Less,
            Token::Symbol(">") => BinaryOperator::Greater,
            Token::Symbol("<=") => BinaryOperator::LessEqual,
            Token::Symbol(">=") => BinaryOperator::GreaterEqual,
            Token::Symbol("~=") => BinaryOperator::NotEqual,
            Token::Symbol("==") => BinaryOperator::Equal,
            Token::Symbol("..") => BinaryOperator::Concat,
            Token::Symbol("+") => BinaryOperator::Add,
            Token::Symbol("-") => BinaryOperator::Subtract,
            Token::Symbol("*") => BinaryOperator::Multiply,
            Token::Symbol("/") => BinaryOperator::Divide,
            Token::Symbol("%") => BinaryOperator::Modulo,
            Token::Symbol("^") => BinaryOperator::Power,
            _ => return None,
        })
    }

    fn parse_unary(&mut self) -> Result<Expression, ParseError> {
        let span = self.span();
        let operator = match self.peek() {
            Token::Symbol("-") => Some(UnaryOperator::Negate),
            Token::Symbol("#") => Some(UnaryOperator::Length),
            Token::Keyword(Keyword::Not) => Some(UnaryOperator::Not),
            _ => None,
        };
        if let Some(operator) = operator {
            self.advance();

            let operand = self.parse_binary(7)?;
            return Ok(Expression::Unary {
                operator,
                operand: Box::new(operand),
                span,
            });
        }
        self.parse_simple_expression()
    }

    fn parse_simple_expression(&mut self) -> Result<Expression, ParseError> {
        let span = self.span();
        match self.peek().clone() {
            Token::Keyword(Keyword::Nil) => {
                self.advance();
                Ok(Expression::Nil(span))
            }
            Token::Keyword(Keyword::True) => {
                self.advance();
                Ok(Expression::Boolean(true, span))
            }
            Token::Keyword(Keyword::False) => {
                self.advance();
                Ok(Expression::Boolean(false, span))
            }
            Token::Number(value) => {
                self.advance();
                Ok(Expression::Number(value, span))
            }
            Token::String(value) => {
                self.advance();
                Ok(Expression::String(value, span))
            }
            Token::Symbol("...") => {
                self.advance();
                Ok(Expression::Varargs(span))
            }
            Token::Keyword(Keyword::Function) => {
                self.advance();
                let body = self.parse_function_body(span, false)?;
                Ok(Expression::Function(Box::new(body)))
            }
            Token::Symbol("{") => self.parse_table_constructor(),
            _ => self.parse_suffixed_expression(),
        }
    }

    fn parse_primary_expression(&mut self) -> Result<Expression, ParseError> {
        let span = self.span();
        match self.peek().clone() {
            Token::Name(name) => {
                self.advance();
                Ok(Expression::Name(name, span))
            }
            Token::Symbol("(") => {
                self.advance();
                let inner = self.parse_expression()?;
                self.expect_symbol(")")?;
                Ok(inner)
            }
            other => Err(self.error(format!(
                "expected an expression, found {}",
                other.describe()
            ))),
        }
    }

    fn parse_suffixed_expression(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_primary_expression()?;
        loop {
            let span = expression.span();
            if self.accept_symbol(".") {
                let name = self.expect_name()?;
                expression = Expression::Index {
                    base: Box::new(expression),
                    key: Box::new(Expression::String(name, span)),
                    span,
                };
                continue;
            }
            if self.accept_symbol("[") {
                let key = self.parse_expression()?;
                self.expect_symbol("]")?;
                expression = Expression::Index {
                    base: Box::new(expression),
                    key: Box::new(key),
                    span,
                };
                continue;
            }
            if self.accept_symbol(":") {
                let method = self.expect_name()?;
                let arguments = self.parse_call_arguments()?;
                expression = Expression::Call {
                    base: Box::new(expression),
                    arguments,
                    method: Some(method),
                    span,
                };
                continue;
            }
            if self.at_call_arguments() {
                let arguments = self.parse_call_arguments()?;
                expression = Expression::Call {
                    base: Box::new(expression),
                    arguments,
                    method: None,
                    span,
                };
                continue;
            }
            return Ok(expression);
        }
    }

    fn at_call_arguments(&self) -> bool {
        self.check_symbol("(") || self.check_symbol("{") || matches!(self.peek(), Token::String(_))
    }

    fn parse_call_arguments(&mut self) -> Result<Vec<Expression>, ParseError> {
        if self.check_symbol("{") {
            return Ok(vec![self.parse_table_constructor()?]);
        }
        if let Token::String(value) = self.peek().clone() {
            let span = self.span();
            self.advance();
            return Ok(vec![Expression::String(value, span)]);
        }
        self.expect_symbol("(")?;
        if self.accept_symbol(")") {
            return Ok(Vec::new());
        }
        let arguments = self.parse_expression_list()?;
        self.expect_symbol(")")?;
        Ok(arguments)
    }

    fn parse_table_constructor(&mut self) -> Result<Expression, ParseError> {
        let span = self.span();
        self.expect_symbol("{")?;
        let mut table = TableConstructor::default();
        loop {
            if self.accept_symbol("}") {
                return Ok(Expression::Table(table, span));
            }
            if self.check_symbol("[") {
                self.advance();
                let key = self.parse_expression()?;
                self.expect_symbol("]")?;
                self.expect_symbol("=")?;
                let value = self.parse_expression()?;
                table.fields.push((key, value));
            } else if matches!(self.peek(), Token::Name(_))
                && self.peek_at(1) == &Token::Symbol("=")
            {
                let name_span = self.span();
                let name = self.expect_name()?;
                self.expect_symbol("=")?;
                let value = self.parse_expression()?;
                table
                    .fields
                    .push((Expression::String(name, name_span), value));
            } else {
                table.items.push(self.parse_expression()?);
            }
            if !self.accept_symbol(",") && !self.accept_symbol(";") {
                self.expect_symbol("}")?;
                return Ok(Expression::Table(table, span));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(source: &str) -> Chunk {
        parse(source, "test").unwrap_or_else(|error| panic!("parses: {error}"))
    }

    #[test]
    fn parses_assignments() {
        let parsed = chunk("x = 1");
        assert_eq!(parsed.body.statements.len(), 1);
        let Statement::Assign {
            targets, values, ..
        } = &parsed.body.statements[0]
        else {
            panic!("expected an assignment")
        };
        assert_eq!(targets.len(), 1);
        assert_eq!(values.len(), 1);
    }

    #[test]
    fn parses_local_declarations() {
        let parsed = chunk("local a, b = 1, 2");
        let Statement::LocalAssign { names, values, .. } = &parsed.body.statements[0] else {
            panic!("expected a local assignment")
        };
        assert_eq!(names, &["a".to_string(), "b".to_string()]);
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn parses_operator_precedence() {
        let parsed = chunk("return 1 + 2 * 3");
        let Some(values) = &parsed.body.ret else {
            panic!("expected a return")
        };
        let Expression::Binary {
            operator, right, ..
        } = &values[0]
        else {
            panic!("expected a binary expression")
        };
        assert_eq!(*operator, BinaryOperator::Add);
        assert!(matches!(
            **right,
            Expression::Binary {
                operator: BinaryOperator::Multiply,
                ..
            }
        ));
    }

    #[test]
    fn power_is_right_associative() {
        let parsed = chunk("return 2 ^ 3 ^ 2");
        let values = parsed.body.ret.unwrap();
        let Expression::Binary { right, .. } = &values[0] else {
            panic!("expected a binary expression")
        };
        assert!(matches!(**right, Expression::Binary { .. }));
    }

    #[test]
    fn concatenation_is_right_associative() {
        let parsed = chunk("return 'a' .. 'b' .. 'c'");
        let values = parsed.body.ret.unwrap();
        let Expression::Binary {
            operator, right, ..
        } = &values[0]
        else {
            panic!("expected a binary expression")
        };
        assert_eq!(*operator, BinaryOperator::Concat);
        assert!(matches!(
            **right,
            Expression::Binary {
                operator: BinaryOperator::Concat,
                ..
            }
        ));
    }

    #[test]
    fn parses_function_definitions_and_calls() {
        let parsed = chunk("function m.f(x) return x end");
        let Statement::FunctionDeclaration { path, method, .. } = &parsed.body.statements[0] else {
            panic!("expected a function declaration")
        };
        assert_eq!(path, &["m".to_string(), "f".to_string()]);
        assert!(!method);

        let parsed = chunk("function m:f(x) return self end");
        let Statement::FunctionDeclaration {
            method, function, ..
        } = &parsed.body.statements[0]
        else {
            panic!("expected a method declaration")
        };
        assert!(method);
        assert_eq!(
            function.parameters,
            vec!["self".to_string(), "x".to_string()]
        );
    }

    #[test]
    fn parses_anonymous_functions_and_varargs() {
        let parsed = chunk("local f = function(a, ...) return a end");
        let Statement::LocalAssign { values, .. } = &parsed.body.statements[0] else {
            panic!("expected a local assignment")
        };
        let Expression::Function(body) = &values[0] else {
            panic!("expected a function")
        };
        assert_eq!(body.parameters, vec!["a".to_string()]);
        assert!(body.varargs);
    }

    #[test]
    fn parses_table_constructors() {
        let parsed = chunk("local t = {1, 2, x = 3, ['y'] = 4}");
        let Statement::LocalAssign { values, .. } = &parsed.body.statements[0] else {
            panic!("expected a local assignment")
        };
        let Expression::Table(table, _) = &values[0] else {
            panic!("expected a table")
        };
        assert_eq!(table.items.len(), 2);
        assert_eq!(table.fields.len(), 2);
    }

    #[test]
    fn parses_control_flow() {
        assert!(matches!(
            chunk("if a then b() elseif c then d() else e() end")
                .body
                .statements[0],
            Statement::If { .. }
        ));
        assert!(matches!(
            chunk("while a do b() end").body.statements[0],
            Statement::While { .. }
        ));
        assert!(matches!(
            chunk("repeat a() until b").body.statements[0],
            Statement::Repeat { .. }
        ));
        assert!(matches!(
            chunk("for i = 1, 10, 2 do a() end").body.statements[0],
            Statement::NumericFor { .. }
        ));
        assert!(matches!(
            chunk("for k, v in pairs(t) do a() end").body.statements[0],
            Statement::GenericFor { .. }
        ));
        assert!(matches!(
            chunk("do local x = 1 end").body.statements[0],
            Statement::Do(_)
        ));
        assert!(matches!(
            chunk("while true do break end").body.statements[0],
            Statement::While { .. }
        ));
    }

    #[test]
    fn parses_method_calls() {
        let parsed = chunk("s:sub(1, 2)");
        let Statement::Call(Expression::Call { method, .. }) = &parsed.body.statements[0] else {
            panic!("expected a method call")
        };
        assert_eq!(method.as_deref(), Some("sub"));
    }

    #[test]
    fn parses_string_call_sugar() {
        let parsed = chunk("print 'hello'");
        let Statement::Call(Expression::Call { arguments, .. }) = &parsed.body.statements[0] else {
            panic!("expected a call")
        };
        assert_eq!(arguments.len(), 1);
    }

    #[test]
    fn parses_return_with_no_values() {
        let parsed = chunk("return");
        assert_eq!(parsed.body.ret, Some(Vec::new()));
    }

    #[test]
    fn parses_semicolons_and_empty_statements() {
        let parsed = chunk(";;a = 1;;");
        assert_eq!(parsed.body.statements.len(), 1);
    }

    #[test]
    fn reports_syntax_errors_with_positions() {
        let error = parse("x = ", "test").unwrap_err();
        assert_eq!(error.span.line, 1);
        assert!(error.message.contains("expected an expression"), "{error}");

        let error = parse("if x then", "test").unwrap_err();
        assert!(error.message.contains("`end`"), "{error}");
    }

    #[test]
    fn rejects_assignment_to_a_call() {
        assert!(parse("f() = 1", "test").is_err());
    }
}
