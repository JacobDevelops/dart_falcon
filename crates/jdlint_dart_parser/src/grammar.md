# Dart 3.x Grammar Reference

This document is the grammar reference for the `jdlint_dart_parser` hand-rolled recursive-descent
parser. Productions marked **[Phase 1]** are fully implemented. Others are noted as partial or
deferred.

---

## 1. Compilation Unit

```
compilationUnit
  : libraryDirective?
    partOfDirective?
    partDirective*
    importDirective*
    exportDirective*
    topLevelDeclaration*
    EOF
  ;

libraryDirective
  : annotation* 'library' dottedIdentifier ';'
  ;

partOfDirective
  : annotation* 'part' 'of' ( stringLiteral | dottedIdentifier ) ';'
  ;

partDirective
  : annotation* 'part' stringLiteral ';'
  ;

importDirective                          // [Phase 1]
  : annotation* 'import' stringLiteral
    ( 'deferred' )? ( 'as' identifier )?
    ( showCombinator | hideCombinator )*
    ';'
  ;

exportDirective                          // [Phase 1]
  : annotation* 'export' stringLiteral
    ( showCombinator | hideCombinator )*
    ';'
  ;

showCombinator : 'show' identifierList ;
hideCombinator : 'hide' identifierList ;
```

---

## 2. Top-Level Declarations

```
topLevelDeclaration
  : classDeclaration
  | mixinDeclaration
  | mixinClassDeclaration
  | enumDeclaration
  | extensionDeclaration
  | extensionTypeDeclaration
  | functionDeclaration
  | topLevelVariableDeclaration
  | typeAliasDeclaration
  ;
```

### 2.1 Class Declaration

```
classDeclaration                         // [Phase 1]
  : annotation*
    ( 'abstract' )? ( 'interface' )? ( 'base' )? ( 'final' )? ( 'sealed' )?
    'class' typeIdentifier typeParameters?
    superclass? withClause? implementsClause?
    '{' classMember* '}'
  ;

superclass : 'extends' type ;
withClause : 'with' typeList ;
implementsClause : 'implements' typeList ;
```

### 2.2 Mixin Declaration

```
mixinDeclaration                         // [Phase 1]
  : annotation* ( 'base' )? 'mixin' typeIdentifier typeParameters?
    onClause? implementsClause?
    '{' classMember* '}'
  ;

mixinClassDeclaration                    // [Phase 1]
  : annotation* ( 'abstract' )? ( 'base' )? 'mixin' 'class' typeIdentifier typeParameters?
    superclass? withClause? implementsClause?
    '{' classMember* '}'
  ;

onClause : 'on' typeList ;
```

### 2.3 Enum Declaration

```
enumDeclaration                          // [Phase 1]
  : annotation* 'enum' typeIdentifier typeParameters?
    withClause? implementsClause?
    '{' enumValue ( ',' enumValue )* ( ',' )? ( ';' classMember* )? '}'
  ;

enumValue
  : annotation* identifier typeArguments? arguments?
  ;
```

### 2.4 Extension Declaration

```
extensionDeclaration                     // [Phase 1]
  : annotation* 'extension' identifier? typeParameters? 'on' type
    '{' classMember* '}'
  ;

extensionTypeDeclaration                 // [Phase 1]
  : annotation* 'extension' 'type' 'const'? identifier typeParameters?
    '(' formalParameterList ')' implementsClause?
    '{' classMember* '}'
  ;
```

### 2.5 Type Alias

```
typeAliasDeclaration                     // [Phase 1]
  : annotation* 'typedef' typeIdentifier typeParameters? '=' type ';'
  | annotation* 'typedef' type identifier formalParameterList ';'  // legacy
  ;
```

### 2.6 Top-Level Variables

```
topLevelVariableDeclaration              // [Phase 1]
  : annotation* ( 'external' )? ( 'late' )? ( 'final' | 'const' )? ( 'var' )?
    type? variableDeclaratorList ';'
  ;
```

### 2.7 Top-Level Functions

```
functionDeclaration                      // [Phase 1]
  : annotation* ( 'external' )? returnType? identifier typeParameters?
    formalParameterList ( 'async' | 'async*' | 'sync*' )? functionBody
  ;
```

---

## 3. Class Members

```
classMember
  : fieldDeclaration
  | constructorDeclaration
  | methodDeclaration
  | getterDeclaration
  | setterDeclaration
  | operatorDeclaration
  ;
```

### 3.1 Field

```
fieldDeclaration                         // [Phase 1]
  : annotation*
    ( 'static' )? ( 'abstract' )? ( 'external' )? ( 'covariant' )?
    ( 'late' )? ( 'final' | 'const' )?
    type? variableDeclaratorList ';'
  ;
```

### 3.2 Constructor

```
constructorDeclaration                   // [Phase 1]
  : annotation*
    ( 'const' )? ( 'factory' )? ( 'external' )?
    identifier ( '.' identifier )?
    formalParameterList
    ( ':' constructorInitializerList )?
    ( 'native' stringLiteral )? functionBody
  ;

constructorInitializerList
  : constructorInitializer ( ',' constructorInitializer )*
  ;

constructorInitializer
  : 'super' ( '.' identifier )? arguments
  | 'this' ( '.' identifier )? ( '=' expression | arguments )
  | identifier '=' expression
  | 'assert' '(' expression ( ',' expression )? ')'
  ;
```

### 3.3 Method

```
methodDeclaration                        // [Phase 1]
  : annotation*
    ( 'static' )? ( 'abstract' )? ( 'external' )? ( 'async' )?
    returnType? identifier typeParameters? formalParameterList
    functionBody
  ;
```

### 3.4 Getter / Setter

```
getterDeclaration                        // [Phase 1]
  : annotation*
    ( 'static' )? ( 'abstract' )? ( 'external' )?
    returnType? 'get' identifier functionBody
  ;

setterDeclaration                        // [Phase 1]
  : annotation*
    ( 'static' )? ( 'abstract' )? ( 'external' )?
    returnType? 'set' identifier formalParameterList functionBody
  ;
```

### 3.5 Operator

```
operatorDeclaration                      // [Phase 1]
  : annotation* ( 'external' )?
    returnType? 'operator' overridableOperator formalParameterList functionBody
  ;

overridableOperator
  : '~' | '+' | '-' | '*' | '/' | '~/' | '%' | '^' | '&' | '|'
  | '<<' | '>>' | '>>>' | '==' | '<' | '>' | '<=' | '>='
  | '[]' | '[]='
  ;
```

---

## 4. Types

```
type                                     // [Phase 1]
  : functionType '?'?
  | typeNotFunction
  ;

typeNotFunction
  : 'void'
  | recordType '?'?
  | typeNotVoidNotFunction '?'?
  ;

typeNotVoidNotFunction
  : typeName typeArguments? '?'?
  | 'Function' '?'?
  ;

typeName : typeIdentifier ( '.' typeIdentifier )* ;

typeArguments : '<' typeList '>' ;
typeList : type ( ',' type )* ;

functionType                             // [Phase 1]
  : functionTypeTails
  | typeNotFunction functionTypeTails
  ;

functionTypeTails
  : functionTypeTail '?'? functionTypeTails?
  ;

functionTypeTail
  : 'Function' typeParameters? parameterTypeList
  ;

parameterTypeList
  : '(' ')'
  | '(' normalParameterTypes ','? ')'
  | '(' normalParameterTypes ',' optionalParameterTypes ')'
  | '(' optionalParameterTypes ')'
  ;

recordType                               // [Phase 1]
  : '(' recordTypeFields ',' recordTypeNamedFields ')'
  | '(' recordTypeFields ','? ')'
  | '(' recordTypeNamedFields ')'
  ;

typeParameters                           // [Phase 1]
  : '<' typeParameter ( ',' typeParameter )* '>'
  ;

typeParameter
  : metadata identifier ( 'extends' typeNotVoidNotFunction )?
  ;
```

---

## 5. Formal Parameters

```
formalParameterList                      // [Phase 1]
  : '(' ')'
  | '(' normalFormalParameters ','? ')'
  | '(' normalFormalParameters ',' optionalOrNamedFormalParameters ')'
  | '(' optionalOrNamedFormalParameters ')'
  ;

optionalOrNamedFormalParameters
  : optionalPositionalFormalParameters
  | namedFormalParameters
  ;

optionalPositionalFormalParameters
  : '[' defaultFormalParameter ( ',' defaultFormalParameter )* ','? ']'
  ;

namedFormalParameters
  : '{' defaultNamedParameter ( ',' defaultNamedParameter )* ','? '}'
  ;

normalFormalParameter
  : metadata ( 'covariant' )? ( 'final' )? ( 'var' )?
    ( 'required' )? type? identifier
  | metadata 'this' '.' identifier ( '=' expression )?
  | metadata 'super' '.' identifier ( '=' expression )?
  | metadata identifier formalParameterList  // function parameter
  ;
```

---

## 6. Statements

```
statement                                // [Phase 1]
  : block
  | localVariableDeclaration
  | localFunctionDeclaration
  | forStatement
  | whileStatement
  | doStatement
  | switchStatement
  | ifStatement
  | rethrowStatement
  | tryStatement
  | breakStatement
  | continueStatement
  | returnStatement
  | yieldStatement
  | yieldEachStatement
  | assertStatement
  | expressionStatement
  ;

block : '{' statement* '}' ;

localVariableDeclaration
  : ( 'late' )? ( 'final' | 'const' | 'var' )?
    type? variableDeclaratorList ';'
  ;

ifStatement
  : 'if' '(' expression ( 'case' guardedPattern )? ')' statement
    ( 'else' statement )?
  ;

forStatement
  : 'await'? 'for' '(' forLoopParts ')' statement
  ;

forLoopParts
  : forInitializerStatement expression? ';' expressionList?    // C-style
  | 'final'? type? identifier 'in' expression                  // for-in
  ;

whileStatement   : 'while' '(' expression ')' statement ;
doStatement      : 'do' statement 'while' '(' expression ')' ';' ;
returnStatement  : 'return' expression? ';' ;
breakStatement   : 'break' identifier? ';' ;
continueStatement: 'continue' identifier? ';' ;
yieldStatement   : 'yield' expression ';' ;
throwStatement   : 'throw' expression ;

switchStatement                          // [Phase 1]
  : 'switch' '(' expression ')' '{' switchStatementCase* switchDefault? '}'
  ;

switchStatementCase
  : label* 'case' expression ':' statement*
  ;

tryStatement
  : 'try' block ( onPart+ finallyPart? | finallyPart )
  ;

onPart
  : catchPart block
  | 'on' type catchPart? block
  ;

catchPart : 'catch' '(' identifier ( ',' identifier )? ')' ;
finallyPart : 'finally' block ;
```

---

## 7. Expressions

Expressions use precedence climbing. The levels from lowest to highest:

```
expression                               // [Phase 1]
  : assignableExpression assignmentOperator expression
  | conditionalExpression
  | throwExpression
  ;

conditionalExpression
  : logicalOrExpression ( '?' expression ':' expression )?
  ;

// Precedence levels (lowest → highest):
//  1. Assignment     : = += -= *= /= %= ~/= &= |= ^= <<= >>= >>>=
//  2. Conditional    : ?:
//  3. Null-coalesce  : ??
//  4. Logical OR     : ||
//  5. Logical AND    : &&
//  6. Bitwise OR     : |
//  7. Bitwise XOR    : ^
//  8. Bitwise AND    : &
//  9. Equality       : == !=
// 10. Relational     : < > <= >= as is is!
// 11. Shift          : << >> >>>
// 12. Additive       : + -
// 13. Multiplicative : * / ~/ %
// 14. Unary prefix   : -e !e ~e ++e --e await
// 15. Postfix        : e++ e-- e() e[] e.id e?.id e!

primaryExpression
  : identifier
  | literal
  | 'this'
  | 'super'
  | 'new' type arguments
  | 'const' type arguments
  | functionExpression
  | '(' expression ')'
  | switchExpression
  ;

switchExpression                         // [Phase 1]
  : 'switch' '(' expression ')' '{'
    ( switchExpressionCase ( ',' switchExpressionCase )* ','? )?
    '}'
  ;

switchExpressionCase : guardedPattern '=>' expression ;
guardedPattern : pattern ( 'when' expression )? ;

cascadeExpression                        // [Phase 1]
  : primaryExpression ( '.' | '?..' ) cascadeSection+
  ;
```

---

## 8. Patterns (Dart 3.x)

```
pattern                                  // [Phase 1]
  : logicalOrPattern
  ;

logicalOrPattern  : logicalAndPattern ( '|' logicalAndPattern )* ;
logicalAndPattern : relationalPattern ( '&' relationalPattern )* ;

relationalPattern
  : ( '<' | '>' | '<=' | '>=' ) bitwiseOrExpression
  | unaryPattern
  ;

unaryPattern
  : castPattern
  | nullAssertPattern
  | nullCheckPattern
  | primaryPattern
  ;

castPattern        : primaryPattern 'as' type ;
nullAssertPattern  : primaryPattern '!' ;
nullCheckPattern   : primaryPattern '?' ;

primaryPattern
  : '_' ( ':' type )?                    // wildcard
  | identifier ( ':' type )?            // variable
  | literal                              // constant
  | 'const' expression                  // const
  | '(' pattern ')'                     // parenthesised
  | listPattern
  | recordPattern
  | mapPattern
  | objectPattern
  ;

listPattern  : '<' type '>'? '[' listPatternElements? ']' ;
recordPattern: '(' patternFields? ')' ;
mapPattern   : '<' type ',' type '>'? '{' mapPatternEntry ( ',' mapPatternEntry )* ','? '}' ;
objectPattern: typeName typeArguments? '(' patternFields? ')' ;
```

---

## 9. Literals

```
literal
  : numericLiteral                       // int, double, hex
  | booleanLiteral                       // true, false
  | nullLiteral                          // null
  | stringLiteral                        // 'x', "x", r'x', '''…''', """…"""
  | symbolLiteral                        // #foo
  | listLiteral                          // [ ... ]
  | setOrMapLiteral                      // { ... }
  | recordLiteral                        // ( a, b, )
  ;

stringLiteral                            // [Phase 1]
  : ( singleLineString | multiLineString )+
  ;

singleLineString
  : '"' stringContent* '"'
  | "'" stringContent* "'"
  | 'r"' [^"]* '"'
  | "r'" [^']* "'"
  ;

multiLineString
  : '"""' stringContent* '"""'
  | "'''" stringContent* "'''"
  ;

stringInterpolation                      // [Phase 1]
  : '$' identifier
  | '${' expression '}'
  ;
```

---

## 10. Annotations

```
annotation
  : '@' qualifiedName typeArguments? ( '.' identifier )? arguments?
  ;
```

---

## 11. Grammar Symbols

```
identifier : IDENTIFIER ;
typeIdentifier : IDENTIFIER ;
dottedIdentifier : identifier ( '.' identifier )* ;
identifierList : identifier ( ',' identifier )* ;
variableDeclaratorList : variableDeclarator ( ',' variableDeclarator )* ;
variableDeclarator : identifier ( '=' expression )? ;
returnType : type | 'void' ;
```

---

## Implementation Notes

- The parser uses **error recovery**: on a syntax error it emits an `ErrorNode` and resumes
  at the next `;` or `}` boundary. No panics on malformed input.
- String interpolation is lexed as a token sequence and re-assembled during parsing.
- Pattern matching (section 8) requires Dart 3.x; Dart 2.x `switch` is handled separately.
- The `grammar.md` grammar scope for Phase 1 corresponds to the
  [`PARSER_GRAMMAR.md`](../../.omc/docs/PARSER_GRAMMAR.md) design document.
