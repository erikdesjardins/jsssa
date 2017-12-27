// https://github.com/babel/babylon/blob/80d5f7592041e96ab672d164276e5f89038ced63/ast/spec.md

macro_rules! node {
    ($($(#[$attr:meta])* struct $name:ident [$tag_name:ident] {
        $($(#[$field_attr:meta])* $field_name:ident: $field_type:ty,)*
    })+) => {
        $(
            #[derive(Serialize, Deserialize, Debug, PartialEq)]
            enum $tag_name { $name }

            #[derive(Serialize, Deserialize, Debug, PartialEq)]
            #[serde(rename_all = "camelCase")]
            $(#[$attr])*
            pub struct $name {
                #[serde(rename = "type")] ty: $tag_name,
                $($(#[$field_attr])* pub $field_name: $field_type,)*
            }

            impl $name {
                #[allow(dead_code)]
                pub fn new($($field_name: $field_type,)*) -> $name {
                    $name {
                        ty: $tag_name::$name,
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
            #[serde(untagged)]
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
    struct File [FileTag] {
        program: Program,
    }

    struct Program [ProgramTag] {
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
    struct Identifier [IdentifierTag] {
        name: String,
    }

    struct RegExpLiteral [RegExpLiteralTag] {
        pattern: String,
        flags: String,
    }

    struct NullLiteral [NullLiteralTag] {}

    struct StringLiteral [StringLiteralTag] {
        value: String,
    }

    struct BooleanLiteral [BooleanLiteralTag] {
        value: bool,
    }

    struct NumericLiteral [NumericLiteralTag] {
        value: f64,
    }

    struct Directive [DirectiveTag] {
        value: DirectiveLiteral,
    }

    struct DirectiveLiteral [DirectiveLiteralTag] {
        value: String,
    }
}

union! {
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
    struct ExpressionStatement [ExpressionStatementTag] {
        expression: Expression,
    }

    struct BlockStatement [BlockStatementTag] {
        body: Vec<Statement>,
        directives: Vec<Directive>,
    }

    struct EmptyStatement [EmptyStatementTag] {}

    struct DebuggerStatement [DebuggerStatementTag] {}

    struct WithStatement [WithStatementTag] {
        object: Expression,
        body: Box<Statement>,
    }

    struct ReturnStatement [ReturnStatementTag] {
        argument: Option<Expression>,
    }

    struct LabeledStatement [LabeledStatementTag] {
        label: Identifier,
        body: Box<Statement>,
    }

    struct BreakStatement [BreakStatementTag] {
        label: Option<Identifier>,
    }

    struct ContinueStatement [ContinueStatementTag] {
        label: Option<Identifier>,
    }

    struct IfStatement [IfStatementTag] {
        test: Expression,
        consequent: Box<Statement>,
        alternate: Option<Box<Statement>>,
    }

    struct SwitchStatement [SwitchStatementTag] {
        discriminant: Expression,
        cases: Vec<SwitchCase>,
    }

    struct SwitchCase [SwitchCaseTag] {
        test: Option<Expression>,
        consequent: Vec<Statement>,
    }

    struct ThrowStatement [ThrowStatementTag] {
        argument: Expression,
    }

    struct TryStatement [TryStatementTag] {
        block: BlockStatement,
        handler: Option<CatchClause>,
        finalizer: Option<BlockStatement>,
    }

    struct CatchClause [CatchClauseTag] {
        param: Option<Pattern>,
        body: BlockStatement,
    }

    struct WhileStatement [WhileStatementTag] {
        test: Expression,
        body: Box<Statement>,
    }

    struct DoWhileStatement [DoWhileStatementTag] {
        body: Box<Statement>,
        test: Expression,
    }

    struct ForStatement [ForStatementTag] {
        init: Option<VarDeclOrExpr>,
        test: Option<Expression>,
        update: Option<Expression>,
        body: Box<Statement>,
    }

    struct ForInStatement [ForInStatementTag] {
        left: VarDeclOrExpr,
        right: Expression,
        body: Box<Statement>,
    }

    struct ForOfStatement [ForOfStatementTag] {
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
    struct FunctionDeclaration [FunctionDeclarationTag] {
        id: Identifier,
        params: Vec<Pattern>,
        body: BlockStatement,
        generator: bool,
        async: bool,
    }

    struct VariableDeclaration [VariableDeclarationTag] {
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
    struct VariableDeclarator [VariableDeclaratorTag] {
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
    struct ThisExpression [ThisExpressionTag] {}

    struct ArrowFunctionExpression [ArrowFunctionExpressionTag] {
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
    struct YieldExpression [YieldExpressionTag] {
        argument: Option<Box<Expression>>,
        delegate: bool,
    }

    struct AwaitExpression [AwaitExpressionTag] {
        argument: Option<Box<Expression>>,
    }

    struct ArrayExpression [ArrayExpressionTag] {
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
    struct ObjectExpression [ObjectExpressionTag] {
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
    struct ObjectProperty [ObjectPropertyTag] {
        key: Expression,
        shorthand: bool,
        value: Expression,
    }

    struct ObjectMethod [ObjectMethodTag] {
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
    struct SpreadElement [SpreadElementTag] {
        argument: Expression,
    }

    struct FunctionExpression [FunctionExpressionTag] {
        id: Option<Identifier>,
        params: Vec<Pattern>,
        body: BlockStatement,
        generator: bool,
        async: bool,
    }

    struct UnaryExpression [UnaryExpressionTag] {
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
    struct UpdateExpression [UpdateExpressionTag] {
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
    struct BinaryExpression [BinaryExpressionTag] {
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
    struct AssignmentExpression [AssignmentExpressionTag] {
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
    struct LogicalExpression [LogicalExpressionTag] {
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
    struct MemberExpression [MemberExpressionTag] {
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
    struct Super [SuperTag] {}

    struct ConditionalExpression [ConditionalExpressionTag] {
        test: Box<Expression>,
        alternate: Box<Expression>,
        consequent: Box<Expression>,
    }

    struct CallExpression [CallExpressionTag] {
        callee: Box<ExprOrSuper>,
        arguments: Vec<ExprOrSpread>,
    }

    struct NewExpression [NewExpressionTag] {
        callee: Box<ExprOrSuper>,
        arguments: Vec<ExprOrSpread>,
    }

    struct SequenceExpression [SequenceExpressionTag] {
        expressions: Vec<Expression>,
    }

    struct TemplateLiteral [TemplateLiteralTag] {
        quasis: Vec<TemplateElement>,
        expressions: Vec<Expression>,
    }

    struct TemplateElement [TemplateElementTag] {
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
    struct TaggedTemplateExpression [TaggedTemplateExpressionTag] {
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
    struct ObjectPattern [ObjectPatternTag] {
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
    struct AssignmentProperty [AssignmentPropertyTag] {
        key: Expression,
        shorthand: bool,
        value: Pattern,
    }

    struct RestElement [RestElementTag] {
        argument: Box<Pattern>,
    }

    struct ArrayPattern [ArrayPatternTag] {
        elements: Vec<Option<Pattern>>,
    }

    struct AssignmentPattern [AssignmentPatternTag] {
        left: Box<Pattern>,
        right: Expression,
    }
}

node! {
    struct ClassDeclaration [ClassDeclarationTag] {
        id: Identifier,
        super_class: Option<Expression>,
        body: ClassBody,
    }

    struct ClassExpression [ClassExpressionTag] {
        id: Option<Identifier>,
        super_class: Option<Box<Expression>>,
        body: ClassBody,
    }

    struct ClassBody [ClassBodyTag] {
        body: Vec<ClassMethod>,
    }

    struct ClassMethod [ClassMethodTag] {
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
