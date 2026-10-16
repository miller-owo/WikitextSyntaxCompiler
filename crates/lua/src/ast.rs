use crate::lex::Span;
#[derive(Clone, PartialEq, Debug)]
pub struct Chunk {
    pub body: Block,
    pub name: String,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub ret: Option<Vec<Expression>>,
}

impl Block {
    pub fn is_empty(&self) -> bool {
        self.statements.is_empty() && self.ret.is_none()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Statement {
    Assign {
        targets: Vec<Expression>,
        values: Vec<Expression>,
        span: Span,
    },

    Call(Expression),

    Do(Block),

    While {
        condition: Expression,
        body: Block,
    },

    Repeat {
        body: Block,
        condition: Expression,
    },

    If {
        condition: Expression,
        then_block: Block,
        else_ifs: Vec<(Expression, Block)>,
        else_block: Option<Block>,
    },

    NumericFor {
        name: String,
        start: Expression,
        stop: Expression,
        step: Option<Expression>,
        body: Block,
    },

    GenericFor {
        names: Vec<String>,
        expressions: Vec<Expression>,
        body: Block,
    },

    FunctionDeclaration {
        path: Vec<String>,
        method: bool,
        function: FunctionBody,
        span: Span,
    },

    LocalAssign {
        names: Vec<String>,
        values: Vec<Expression>,
        span: Span,
    },

    LocalFunction {
        name: String,
        function: FunctionBody,
        span: Span,
    },

    Break(Span),
}

#[derive(Clone, PartialEq, Debug)]
pub struct FunctionBody {
    pub parameters: Vec<String>,
    pub varargs: bool,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Expression {
    Nil(Span),

    Boolean(bool, Span),

    Number(f64, Span),

    String(String, Span),

    Varargs(Span),

    Name(String, Span),

    Function(Box<FunctionBody>),

    Table(TableConstructor, Span),

    Index {
        base: Box<Expression>,
        key: Box<Expression>,
        span: Span,
    },

    Call {
        base: Box<Expression>,
        arguments: Vec<Expression>,
        method: Option<String>,
        span: Span,
    },

    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
        span: Span,
    },

    Binary {
        operator: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
        span: Span,
    },
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Nil(span)
            | Expression::Boolean(_, span)
            | Expression::Number(_, span)
            | Expression::String(_, span)
            | Expression::Varargs(span)
            | Expression::Name(_, span) => *span,
            Expression::Function(body) => body.span,
            Expression::Table(_, span)
            | Expression::Index { span, .. }
            | Expression::Call { span, .. }
            | Expression::Unary { span, .. }
            | Expression::Binary { span, .. } => *span,
        }
    }

    pub fn is_assignable(&self) -> bool {
        matches!(self, Expression::Name(..) | Expression::Index { .. })
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct TableConstructor {
    pub items: Vec<Expression>,
    pub fields: Vec<(Expression, Expression)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnaryOperator {
    Negate,
    Not,
    Length,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BinaryOperator {
    Or,
    And,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    NotEqual,
    Equal,
    Concat,
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Power,
}

impl BinaryOperator {
    pub const fn as_str(self) -> &'static str {
        match self {
            BinaryOperator::Or => "or",
            BinaryOperator::And => "and",
            BinaryOperator::Less => "<",
            BinaryOperator::Greater => ">",
            BinaryOperator::LessEqual => "<=",
            BinaryOperator::GreaterEqual => ">=",
            BinaryOperator::NotEqual => "~=",
            BinaryOperator::Equal => "==",
            BinaryOperator::Concat => "..",
            BinaryOperator::Add => "+",
            BinaryOperator::Subtract => "-",
            BinaryOperator::Multiply => "*",
            BinaryOperator::Divide => "/",
            BinaryOperator::Modulo => "%",
            BinaryOperator::Power => "^",
        }
    }

    pub const fn precedence(self) -> u8 {
        match self {
            BinaryOperator::Or => 1,
            BinaryOperator::And => 2,
            BinaryOperator::Less
            | BinaryOperator::Greater
            | BinaryOperator::LessEqual
            | BinaryOperator::GreaterEqual
            | BinaryOperator::NotEqual
            | BinaryOperator::Equal => 3,
            BinaryOperator::Concat => 4,
            BinaryOperator::Add | BinaryOperator::Subtract => 5,
            BinaryOperator::Multiply | BinaryOperator::Divide | BinaryOperator::Modulo => 6,
            BinaryOperator::Power => 8,
        }
    }
}
