//! https://github.com/babel/babylon/blob/80d5f7592041e96ab672d164276e5f89038ced63/ast/spec.md

use serde::{Serialize, Serializer};
use serde::ser::SerializeStruct;

macro_rules! count {
    () => ( 0 );
    ($x:ident $($xs:ident)*) => ( 1 + count!($($xs)*) );
}

macro_rules! node {
    ($($(#[$attr:meta])* struct $name:ident {
        $($(#[$field_attr:meta])* $field_name:ident: $field_type:ty,)*
    })+) => {
        $(
            #[derive(Deserialize, Debug, PartialEq)]
            #[serde(rename_all = "camelCase")]
            $(#[$attr])*
            pub struct $name {
                $($(#[$field_attr])* pub $field_name: $field_type,)*
            }

            impl Serialize for $name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
                    let mut s = Serializer::serialize_struct(
                        serializer,
                        stringify!($name),
                        1 /* type */ + count!($($field_name)*)
                    )?;
                    s.serialize_field("type", stringify!($name))?;
                    $(s.serialize_field(stringify!($field_name), &self.$field_name)?;)*
                    s.end()
                }
            }

            impl $name {
                #[allow(dead_code)]
                pub fn new($($field_name: $field_type,)*) -> $name {
                    $name {
                        $($field_name: $field_name,)*
                    }
                }
            }
        )+
    };
}

macro_rules! union {
    ($($(#[$attr:meta])* enum $name:ident {
        $($(#[$variant_attr:meta])* $variant_type:ident,)*
    })+) => {
        $(
            #[derive(Serialize, Deserialize, Debug, PartialEq)]
            #[serde(tag = "type")]
            $(#[$attr])*
            pub enum $name {
                $($(#[$variant_attr])* $variant_type($variant_type),)*
            }

            $(
                impl From<$variant_type> for $name {
                    fn from(variant: $variant_type) -> $name {
                        $name::$variant_type(variant)
                    }
                }
            )*
        )+
    }
}

macro_rules! string_enum {
    ($($(#[$attr:meta])* enum $name:ident {
        $($(#[$variant_attr:meta])* $variant_name:ident,)*
    })+) => {
        $(
            #[derive(Serialize, Deserialize, Debug, PartialEq)]
            #[serde(rename_all = "camelCase")]
            $(#[$attr])*
            pub enum $name {
                $($(#[$variant_attr])* $variant_name,)*
            }
        )+
    }
}

node! {
    struct File {
        program: Program,
    }

    struct Program {
        body: Vec<Statement>,
        directives: Vec<Directive>,
        source_type: SourceType,
    }
}

string_enum! {
    enum SourceType {
        Script,
        Module,
    }
}

node! {
    struct Identifier {
        name: String,
    }

    struct RegExpLiteral {
        pattern: String,
        flags: String,
    }

    struct NullLiteral {}

    struct StringLiteral {
        value: String,
    }

    struct BooleanLiteral {
        value: bool,
    }

    struct NumericLiteral {
        value: f64,
    }

    struct Directive {
        value: DirectiveLiteral,
    }

    struct DirectiveLiteral {
        value: String,
    }
}

union! {
    #[allow(large_enum_variant)]
    enum Statement {
        ExpressionStatement,
        BlockStatement,
        EmptyStatement,
        DebuggerStatement,
        WithStatement,
        ReturnStatement,
        LabeledStatement,
        BreakStatement,
        ContinueStatement,
        IfStatement,
        SwitchStatement,
        ThrowStatement,
        TryStatement,
        WhileStatement,
        DoWhileStatement,
        ForStatement,
        ForInStatement,
        ForOfStatement,
        FunctionDeclaration,
        VariableDeclaration,
        ClassDeclaration,
    }
}

node! {
    struct ExpressionStatement {
        expression: Expression,
    }

    struct BlockStatement {
        body: Vec<Statement>,
        directives: Vec<Directive>,
    }

    struct EmptyStatement {}

    struct DebuggerStatement {}

    struct WithStatement {
        object: Expression,
        body: Box<Statement>,
    }

    struct ReturnStatement {
        argument: Option<Expression>,
    }

    struct LabeledStatement {
        label: Identifier,
        body: Box<Statement>,
    }

    struct BreakStatement {
        label: Option<Identifier>,
    }

    struct ContinueStatement {
        label: Option<Identifier>,
    }

    struct IfStatement {
        test: Expression,
        consequent: Box<Statement>,
        alternate: Option<Box<Statement>>,
    }

    struct SwitchStatement {
        discriminant: Expression,
        cases: Vec<SwitchCase>,
    }

    struct SwitchCase {
        test: Option<Expression>,
        consequent: Vec<Statement>,
    }

    struct ThrowStatement {
        argument: Expression,
    }

    struct TryStatement {
        block: BlockStatement,
        handler: Option<CatchClause>,
        finalizer: Option<BlockStatement>,
    }

    struct CatchClause {
        param: Option<Pattern>,
        body: BlockStatement,
    }

    struct WhileStatement {
        test: Expression,
        body: Box<Statement>,
    }

    struct DoWhileStatement {
        body: Box<Statement>,
        test: Expression,
    }

    struct ForStatement {
        init: Option<VarDeclOrExpr>,
        test: Option<Expression>,
        update: Option<Expression>,
        body: Box<Statement>,
    }

    struct ForInStatement {
        left: VarDeclOrExpr,
        right: Expression,
        body: Box<Statement>,
    }

    struct ForOfStatement {
        left: VarDeclOrExpr,
        right: Expression,
        body: Box<Statement>,
        await: bool,
    }
}

union! {
    enum VarDeclOrExpr {
        VariableDeclaration,
        Expression,
    }
}

node! {
    struct FunctionDeclaration {
        id: Identifier,
        params: Vec<Pattern>,
        body: BlockStatement,
        generator: bool,
        async: bool,
    }

    struct VariableDeclaration {
        kind: VariableKind,
        declarations: Vec<VariableDeclarator>,
    }
}

string_enum! {
    enum VariableKind {
        Var,
        Let,
        Const,
    }
}

node! {
    struct VariableDeclarator {
        id: Pattern,
        init: Option<Expression>,
    }
}

union! {
    enum Expression {
        Identifier,
        RegExpLiteral,
        NullLiteral,
        StringLiteral,
        BooleanLiteral,
        NumericLiteral,
        ThisExpression,
        ArrowFunctionExpression,
        YieldExpression,
        AwaitExpression,
        ArrayExpression,
        ObjectExpression,
        FunctionExpression,
        UnaryExpression,
        UpdateExpression,
        BinaryExpression,
        AssignmentExpression,
        LogicalExpression,
        MemberExpression,
        ConditionalExpression,
        CallExpression,
        NewExpression,
        SequenceExpression,
        TemplateLiteral,
        TaggedTemplateExpression,
        ClassExpression,
    }
}

node! {
    struct ThisExpression {}

    struct ArrowFunctionExpression {
        params: Vec<Pattern>,
        body: Box<BlockStmtOrExpr>,
        async: bool,
    }
}

union! {
    enum BlockStmtOrExpr {
        BlockStatement,
        Expression,
    }
}

node! {
    struct YieldExpression {
        argument: Option<Box<Expression>>,
        delegate: bool,
    }

    struct AwaitExpression {
        argument: Option<Box<Expression>>,
    }

    struct ArrayExpression {
        elements: Vec<Option<ExprOrSpread>>,
    }
}

union! {
    enum ExprOrSpread {
        Expression,
        SpreadElement,
    }
}

node! {
    struct ObjectExpression {
        properties: Vec<PropOrMethodOrSpread>,
    }
}

union! {
    enum PropOrMethodOrSpread {
        ObjectProperty,
        ObjectMethod,
        SpreadElement,
    }
}

node! {
    struct ObjectProperty {
        key: Expression,
        shorthand: bool,
        value: Expression,
    }

    struct ObjectMethod {
        kind: ObjectMethodKind,
        key: Expression,
        params: Vec<Pattern>,
        body: BlockStatement,
        generator: bool,
        async: bool,
    }
}

string_enum! {
    enum ObjectMethodKind {
        Get,
        Set,
        Method,
    }
}

node! {
    struct SpreadElement {
        argument: Expression,
    }

    struct FunctionExpression {
        id: Option<Identifier>,
        params: Vec<Pattern>,
        body: BlockStatement,
        generator: bool,
        async: bool,
    }

    struct UnaryExpression {
        operator: UnaryOperator,
        prefix: bool,
        argument: Box<Expression>,
    }
}

string_enum! {
    enum UnaryOperator {
        #[serde(rename = "+")] Plus,
        #[serde(rename = "-")] Minus,
        #[serde(rename = "!")] Not,
        #[serde(rename = "~")] Tilde,
        Typeof,
        Void,
        Delete,
    }
}

node! {
    struct UpdateExpression {
        operator: UpdateOperator,
        argument: Box<Expression>,
        prefix: bool,
    }
}

string_enum! {
    enum UpdateOperator {
        #[serde(rename = "++")] Incr,
        #[serde(rename = "--")] Decr,
    }
}

node! {
    struct BinaryExpression {
        operator: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
    }
}

string_enum! {
    enum BinaryOperator {
        #[serde(rename = "==")] Eq,
        #[serde(rename = "!=")] NotEq,
        #[serde(rename = "===")] StrictEq,
        #[serde(rename = "!==")] NotStrictEq,
        #[serde(rename = "<")] LT,
        #[serde(rename = "<=")] LTE,
        #[serde(rename = ">")] GT,
        #[serde(rename = ">=")] GTE,
        #[serde(rename = "<<")] ShiftLeft,
        #[serde(rename = ">>")] ShiftRight,
        #[serde(rename = ">>>")] ShiftRightZero,
        #[serde(rename = "+")] Add,
        #[serde(rename = "-")] Sub,
        #[serde(rename = "*")] Mul,
        #[serde(rename = "/")] Div,
        #[serde(rename = "%")] Mod,
        #[serde(rename = "|")] BitOr,
        #[serde(rename = "^")] BitXor,
        #[serde(rename = "&")] BitAnd,
        In,
        Instanceof,
    }
}

node! {
    struct AssignmentExpression {
        operator: AssignmentOperator,
        left: Box<PatOrExpr>,
        right: Box<Expression>,
    }
}

string_enum! {
    enum AssignmentOperator {
        #[serde(rename = "=")] Eq,
        #[serde(rename = "+=")] AddEq,
        #[serde(rename = "-=")] SubEq,
        #[serde(rename = "*=")] MulEq,
        #[serde(rename = "/=")] DivEq,
        #[serde(rename = "%=")] ModEq,
        #[serde(rename = "<<=")] ShiftLeftEq,
        #[serde(rename = ">>=")] ShiftRightEq,
        #[serde(rename = ">>>=")] ShiftRightZeroEq,
        #[serde(rename = "|=")] BitOrEq,
        #[serde(rename = "^=")] BitXorEq,
        #[serde(rename = "&=")] BitAndEq,
    }
}

union! {
    enum PatOrExpr {
        Pattern,
        Expression,
    }
}

node! {
    struct LogicalExpression {
        operator: LogicalOperator,
        left: Box<Expression>,
        right: Box<Expression>,
    }
}

string_enum! {
    enum LogicalOperator {
        #[serde(rename = "||")] Or,
        #[serde(rename = "&&")] And,
    }
}

node! {
    struct MemberExpression {
        object: Box<ExprOrSuper>,
        property: Box<Expression>,
    }
}

union! {
    enum ExprOrSuper {
        Expression,
        Super,
    }
}

node! {
    struct Super {}

    struct ConditionalExpression {
        test: Box<Expression>,
        alternate: Box<Expression>,
        consequent: Box<Expression>,
    }

    struct CallExpression {
        callee: Box<ExprOrSuper>,
        arguments: Vec<ExprOrSpread>,
    }

    struct NewExpression {
        callee: Box<ExprOrSuper>,
        arguments: Vec<ExprOrSpread>,
    }

    struct SequenceExpression {
        expressions: Vec<Expression>,
    }

    struct TemplateLiteral {
        quasis: Vec<TemplateElement>,
        expressions: Vec<Expression>,
    }

    struct TemplateElement {
        tail: bool,
        value: TemplateElementInner,
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct TemplateElementInner {
    cooked: Option<String>,
    raw: String,
}

node! {
    struct TaggedTemplateExpression {
        tag: Box<Expression>,
        quasi: TemplateLiteral,
    }
}

union! {
    enum Pattern {
        Identifier,
        MemberExpression,
        ObjectPattern,
        ArrayPattern,
        RestElement,
        AssignmentPattern,
    }
}

node! {
    struct ObjectPattern {
        properties: Vec<AssignOrRest>,
    }
}

string_enum! {
    enum AssignOrRest {
        AssignmentProperty,
        RestElement,
    }
}

node! {
    struct AssignmentProperty {
        key: Expression,
        shorthand: bool,
        value: Pattern,
    }

    struct RestElement {
        argument: Box<Pattern>,
    }

    struct ArrayPattern {
        elements: Vec<Option<Pattern>>,
    }

    struct AssignmentPattern {
        left: Box<Pattern>,
        right: Expression,
    }
}

node! {
    struct ClassDeclaration {
        id: Identifier,
        super_class: Option<Expression>,
        body: ClassBody,
    }

    struct ClassExpression {
        id: Option<Identifier>,
        super_class: Option<Box<Expression>>,
        body: ClassBody,
    }

    struct ClassBody {
        body: Vec<ClassMethod>,
    }

    struct ClassMethod {
        kind: ClassMethodKind,
        key: Expression,
        params: Vec<Pattern>,
        body: BlockStatement,
        generator: bool,
        async: bool,
    }
}

string_enum! {
    enum ClassMethodKind {
        Constructor,
        Method,
        Get,
        Set,
    }
}
