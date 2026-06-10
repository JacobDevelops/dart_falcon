# falcon Phase 1: Dart 3.x Parser Grammar Specification

**Date:** 2026-06-09  
**Status:** LOCKED FOR M1 IMPLEMENTATION  
**Target Parser Engineer:** M1 Executor (Weeks 2-3)  
**Audience:** M1 parser implementer; rule engineers (M4+) who depend on AST structure

---

## 1. Dart 3.x Features: SUPPORTED in Phase 1

The falcon Phase 1 parser MUST handle all Dart 3.x constructs listed below. These are required for Phase 1 lint rules to function correctly. Each production includes grammar notation and notes on which Phase 1 rules depend on it.

### 1.1 Compilation Unit & File Structure

**Grammar Production:**
```
compilation_unit
  : library_directive?
    import_or_export_directive*
    top_level_declaration*
  ;

library_directive
  : 'library' identifier ';'
  ;

import_or_export_directive
  : import_directive
  | export_directive
  ;

import_directive
  : 'import' string_literal ('as' identifier)? import_spec* ';'
  ;

export_directive
  : 'export' string_literal export_spec* ';'
  ;

import_spec
  : 'show' identifier (',' identifier)*
  | 'hide' identifier (',' identifier)*
  ;

export_spec
  : 'show' identifier (',' identifier)*
  | 'hide' identifier (',' identifier)*
  ;

top_level_declaration
  : class_declaration
  | mixin_declaration
  | mixin_class_declaration
  | enum_declaration
  | extension_declaration
  | extension_type_declaration
  | function_declaration
  | variable_declaration
  | metadata annotation+
  ;
```

**Examples:**
```dart
// library_directive
library my_app;

// import_directive
import 'package:flutter/material.dart';
import 'package:provider/provider.dart' as provider;
import 'package:flutter/material.dart' show Widget, State, BuildContext;
import 'package:flutter/material.dart' hide Material;

// export_directive
export 'src/models/user.dart';
export 'src/services/api.dart' show ApiClient;

// top_level_declaration
void main() => runApp(MyApp());
const appName = "falcon";
@Deprecated('Use newFunction instead')
void oldFunction() {}
```

**Phase 1 Rules Requiring This:**
- `unnecessary-flutter-imports` — detects unnecessary package imports
- All rules — require file structure parsing for context

**Priority:** CRITICAL

---

### 1.2 Classes: Standard, Abstract, Interface, Base, Final, Sealed

**Grammar Production:**
```
class_declaration
  : class_modifier* 'class' identifier type_parameters?
    superclass? mixins? interfaces? '{'
      class_body
    '}'
  ;

class_modifier
  : 'abstract'
  | 'interface'
  | 'base'
  | 'final'
  | 'sealed'
  ;

superclass
  : 'extends' type_reference
  ;

mixins
  : 'with' type_reference (',' type_reference)*
  ;

interfaces
  : 'implements' type_reference (',' type_reference)*
  ;

class_body
  : class_member*
  ;

class_member
  : constructor_declaration
  | method_declaration
  | variable_declaration
  | getter_declaration
  | setter_declaration
  ;
```

**Examples:**
```dart
// standard class
class User {
  final String name;
  User(this.name);
}

// abstract class
abstract class Animal {
  void makeSound();
}

// interface class (Dart 3.0)
interface class Shape {
  double area();
}

// base class (Dart 3.0)
base class Vehicle {
  void drive();
}

// final class (Dart 3.0)
final class ImmutableConfig {
  final String value;
  const ImmutableConfig(this.value);
}

// sealed class (Dart 3.0)
sealed class Result {
  const Result();
}

class Success extends Result {
  final dynamic data;
  Success(this.data);
}

class Error extends Result {
  final String message;
  Error(this.message);
}

// class with generics
class Container<T> {
  T _value;
  T get value => _value;
}

// class with mixins and interfaces
class MyWidget extends StatelessWidget with ChangeNotifier implements Listenable {
  @override
  Widget build(BuildContext context) => Text('Hello');
}
```

**Phase 1 Rules Requiring This:**
- `member-ordering` — checks class member declaration order
- `avoid-dynamic` — detects dynamic types in class fields
- `no-equal-arguments` — checks constructor argument duplicates
- `newline-before-return` — enforces spacing in methods
- `prefer-trailing-comma` — checks class member trailing commas

**Priority:** CRITICAL

---

### 1.3 Mixins: Mixin, Mixin Class

**Grammar Production:**
```
mixin_declaration
  : 'mixin' identifier type_parameters?
    ('on' type_reference (',' type_reference)*)?
    ('implements' type_reference (',' type_reference)*)?
    '{'
      mixin_body
    '}'
  ;

mixin_class_declaration
  : 'mixin class' identifier type_parameters?
    superclass? mixins? interfaces? '{'
      mixin_body
    '}'
  ;

mixin_body
  : (method_declaration | variable_declaration | getter_declaration | setter_declaration)*
  ;
```

**Examples:**
```dart
// simple mixin
mixin Copyable<T> {
  T copy();
}

// mixin with 'on' constraint (Dart 2.1+)
mixin ChangeNotifierMixin on ChangeNotifier {
  void notifyListeners() {
    super.notifyListeners();
  }
}

// mixin class (Dart 3.0)
mixin class Observable {
  List<Function()> _listeners = [];
  void addListener(Function() callback) => _listeners.add(callback);
  void notify() => _listeners.forEach((cb) => cb());
}

// using mixin
class MyClass with Copyable<MyClass> {
  MyClass copy() => MyClass();
}
```

**Phase 1 Rules Requiring This:**
- `member-ordering` — checks mixin member order
- All rules — structural understanding for context

**Priority:** HIGH

---

### 1.4 Enums: Enhanced Enums with Members & Implements

**Grammar Production:**
```
enum_declaration
  : 'enum' identifier type_parameters?
    ('implements' type_reference (',' type_reference)*)?
    '{'
      enum_body
    '}'
  ;

enum_body
  : enum_constant (',' enum_constant)* ','?
    (';' enum_member*)?
  ;

enum_constant
  : identifier ('(' argument_list ')')?
  ;

enum_member
  : method_declaration
  | constructor_declaration
  | variable_declaration
  | getter_declaration
  | setter_declaration
  ;
```

**Examples:**
```dart
// simple enum
enum Status { pending, approved, rejected }

// enhanced enum (Dart 2.17+)
enum Color {
  red(0xFF0000),
  green(0x00FF00),
  blue(0x0000FF);

  final int value;
  Color(this.value);
  
  String get hexString => '0x${value.toRadixString(16)}';
}

// enum with methods
enum Priority {
  low,
  medium,
  high;
  
  bool get isUrgent => this == high;
  
  String label() {
    switch (this) {
      case low: return 'Low Priority';
      case medium: return 'Medium Priority';
      case high: return 'High Priority';
    }
  }
}

// enum implementing interface
enum HttpMethod implements Comparable<HttpMethod> {
  get, post, put, delete;
  
  @override
  int compareTo(HttpMethod other) => name.compareTo(other.name);
}
```

**Phase 1 Rules Requiring This:**
- `member-ordering` — checks enum member order (Dart 3.0+ concern)
- `avoid-dynamic` — detects dynamic types in enum fields
- `correct-order-for-super-dispose` — applies to enum dispose patterns

**Priority:** HIGH

---

### 1.5 Extensions & Extension Types

**Grammar Production:**
```
extension_declaration
  : 'extension' identifier? type_parameters?
    'on' type_reference
    '{'
      extension_body
    '}'
  ;

extension_type_declaration
  : 'extension type' ('const')? identifier type_parameters?
    ('implements' type_reference (',' type_reference)*)?
    '=' type_reference
    '{'
      extension_type_body
    '}'
  ;

extension_body
  : (method_declaration | getter_declaration | setter_declaration)*
  ;

extension_type_body
  : (method_declaration | constructor_declaration | getter_declaration | setter_declaration)*
  ;
```

**Examples:**
```dart
// simple extension
extension StringExtension on String {
  bool get isNumeric => this.isNotEmpty && double.tryParse(this) != null;
  
  String get capitalized => '${this[0].toUpperCase()}${this.substring(1)}';
}

// extension with type parameter
extension ListExtension<T> on List<T> {
  T? get secondOrNull => length > 1 ? this[1] : null;
  
  List<T> get reversed => List.from(this.reversed);
}

// extension type (Dart 3.3)
extension type const UserId(int value) {
  bool get isValid => value > 0;
  
  String toString() => 'UserId($value)';
}

// using extensions
void main() {
  print('123'.isNumeric);  // true
  print('hello'.capitalized);  // Hello
  print([1, 2, 3].secondOrNull);  // 2
  
  final id = UserId(42);
  print(id.isValid);  // true
}
```

**Phase 1 Rules Requiring This:**
- `member-ordering` — checks extension member order
- `prefer-trailing-comma` — enforces trailing commas in extension declarations
- All rules — understand extension structure for context

**Priority:** HIGH

---

### 1.6 Functions & Methods: Sync, Async, Generators

**Grammar Production:**
```
function_declaration
  : metadata* type_reference? identifier type_parameters?
    '(' formal_parameter_list? ')'
    function_body
  ;

method_declaration
  : metadata* ('external')? ('static')? ('factory')?
    type_reference? identifier type_parameters?
    '(' formal_parameter_list? ')'
    function_body
  ;

constructor_declaration
  : metadata* identifier ('.' identifier)?
    '(' formal_parameter_list? ')'
    (':' initializer_list)?
    function_body?
  ;

function_body
  : '=>' expression ';'
  | ('async' | 'async*' | 'sync*')? block
  ;

formal_parameter_list
  : positional_formal_parameter (',' positional_formal_parameter)*
    (',' named_formal_parameter (',' named_formal_parameter)*)?
  | named_formal_parameter (',' named_formal_parameter)*
  ;

positional_formal_parameter
  : metadata* ('final' | 'var' | type_reference)? identifier
    ('=' expression)?
  ;

named_formal_parameter
  : metadata* ('required')? ('covariant')?
    ('final' | 'var' | type_reference)? identifier
    ('=' expression)?
  ;

initializer_list
  : initializer (',' initializer)*
  ;

initializer
  : 'super' ('.' identifier)? '(' argument_list? ')'
  | 'this' '.' identifier '=' expression
  | identifier '=' expression
  ;
```

**Examples:**
```dart
// simple function
int add(int a, int b) => a + b;

// function with multiple parameter types
void processData(
  String name,
  {required int age, bool isActive = true, String? nickname}
) {
  print('$name, $age, $isActive, $nickname');
}

// async function
Future<String> fetchData(String url) async {
  final response = await http.get(Uri.parse(url));
  return response.body;
}

// async* generator
Stream<int> countUp(int n) async* {
  for (int i = 0; i <= n; i++) {
    await Future.delayed(Duration(seconds: 1));
    yield i;
  }
}

// sync* generator
Iterable<int> range(int start, int end) sync* {
  for (int i = start; i <= end; i++) {
    yield i;
  }
}

// method with initializer list
class Point {
  int x, y;
  
  Point(int x, int y) : this.x = x, this.y = y;
  
  Point.origin() : x = 0, y = 0;
  
  Point.copy(Point other) : x = other.x, y = other.y;
}

// method with static, factory
class Logger {
  static final Logger _instance = Logger._internal();
  
  Logger._internal();
  
  factory Logger() => _instance;
  
  void log(String message) => print(message);
}

// covariant parameters
abstract class Animal {
  void chase(Animal other);
}

class Dog extends Animal {
  @override
  void chase(covariant Dog other) {
    print('Chasing another dog');
  }
}
```

**Phase 1 Rules Requiring This:**
- `avoid-redundant-async` — detects unnecessary async on functions
- `max-lines-for-function` — enforces function length limits
- `avoid-unused-parameters` — detects unused parameters
- `newline-before-return` — enforces spacing before return
- `prefer-immediate-return` — suggests immediate returns
- `avoid-non-null-assertion` — detects unnecessary ! operators

**Priority:** CRITICAL

---

### 1.7 Variables: var, final, const, late, required

**Grammar Production:**
```
variable_declaration
  : metadata* variable_declaration_statement ';'
  ;

variable_declaration_statement
  : ('final' | 'const' | 'late')? type_reference? identifier
    ('=' initializer_expression)?
    (',' identifier ('=' initializer_expression)?)*
  | 'var' identifier ('=' initializer_expression)?
    (',' identifier ('=' initializer_expression)?)*
  ;
```

**Examples:**
```dart
// var declaration
var count = 5;
var name = 'Alice';

// final declaration
final int age = 25;
final configuration = loadConfig();

// const declaration (compile-time constant)
const String apiUrl = 'https://api.example.com';
const List<int> values = [1, 2, 3];

// late keyword (Dart 2.12+)
late String lazyValue;  // not initialized yet

late final String expensiveString = _computeExpensiveString();

void initializeLate() {
  lazyValue = 'now initialized';
}

// required keyword (named parameters)
void showDialog({
  required String title,
  required String content,
  String? positiveButton,
  String? negativeButton,
}) {
  // ...
}

// class fields with various modifiers
class Configuration {
  var simpleProp = 0;
  final int immutable = 10;
  const String constant = 'constant';
  late String lazyInit;
  
  void setLazy(String value) {
    lazyInit = value;
  }
}
```

**Phase 1 Rules Requiring This:**
- `avoid-dynamic` — detects `var` without type inference
- `avoid-late-keyword` — flags use of `late` keyword
- `no-magic-number` — detects numeric literals in variable initialization
- `prefer-const-constructor` — suggests `const` for static instances

**Priority:** CRITICAL

---

### 1.8 Types: Nullable Types, Function Types, Generics, Records, Never, Dynamic, Object, Void

**Grammar Production:**
```
type_reference
  : nullable_type
  | non_nullable_type
  ;

non_nullable_type
  : 'void'
  | 'dynamic'
  | 'Never'
  | 'Object'
  | identifier type_arguments?
  | function_type
  | record_type
  ;

nullable_type
  : non_nullable_type '?'
  ;

type_arguments
  : '<' type_reference (',' type_reference)* '>'
  ;

type_parameters
  : '<' type_parameter (',' type_parameter)* '>'
  ;

type_parameter
  : identifier ('extends' type_reference)?
  ;

function_type
  : return_type 'Function' type_parameters?
    '(' formal_type_list? ')'
  ;

formal_type_list
  : positional_type (',' positional_type)*
    (',' named_type (',' named_type)*)?
  | named_type (',' named_type)*
  ;

positional_type
  : type_reference
  ;

named_type
  : identifier ':' type_reference
  ;

record_type
  : '(' positional_record_type_field (',' positional_record_type_field)*
      (',' named_record_type_field (',' named_record_type_field)*)? ','? ')'
  | '(' named_record_type_field (',' named_record_type_field)* ','? ')'
  ;

positional_record_type_field
  : type_reference
  ;

named_record_type_field
  : type_reference identifier
  ;
```

**Examples:**
```dart
// nullable types (Dart 2.12+)
String? nullableString;
List<int>? nullableList;
int? optionalNumber;

// function types
int Function(int, int) add;
void Function(String) callback;
Future<String> Function(String) asyncFetch;

// generic types
List<String> names = [];
Map<String, int> scores = {};
Set<DateTime> dates = {};

// nested generics
Map<String, List<Map<String, dynamic>>> complexData = {};

// function type with named parameters
void Function({required String name, int age}) processUser;

// record types (Dart 3.0+)
(String, int) pair;
({String name, int age}) record;
(String, int, {bool active}) mixed;

// Never type (Dart 2.15+)
Never throwError(String message) => throw Exception(message);

// dynamic type
dynamic value = 42;
dynamic anotherValue = 'string';

// Object type
Object obj = 'anything';

// void type
void doNothing() {}

// complex function type
Future<void> Function({
  required String endpoint,
  required Map<String, dynamic> body,
  Duration? timeout,
}) apiRequest;
```

**Phase 1 Rules Requiring This:**
- `avoid-dynamic` — detects `dynamic` types
- `avoid-unnecessary-type-casts` — detects redundant type casts
- `avoid-unnecessary-type-assertions` — detects redundant type assertions
- `no-equal-arguments` — type comparison in arguments
- `prefer-iterable-of` — detects untyped iterables

**Priority:** CRITICAL

---

### 1.9 Null-Safety Operators: ?., ??, ??=, !

**Grammar Production:**
```
postfix_expression
  : primary_expression postfix_operator*
  ;

postfix_operator
  : '.'
  | '?.'
  | call_suffix
  | array_suffix
  ;

null_coalescing_operator
  : '??'
  | '??='
  ;

bang_operator
  : '!'
  ;

expression
  : assignment_expression
  | conditional_expression null_coalescing_expression*
  ;

null_coalescing_expression
  : null_coalescing_operator conditional_expression
  ;
```

**Examples:**
```dart
// null conditional access (?.)
String? maybeString;
int? length = maybeString?.length;

// null coalescing (??)
String name = maybeString ?? 'default';

// null coalescing assignment (??=)
maybeString ??= 'fallback';

// non-null assertion (!)
String required = maybeString!;

// chaining null operators
String? result = obj?.property?.method()?.toString() ?? 'fallback';

// in collections
List<String?> items = ['a', null, 'c'];
List<String> nonNull = items.whereType<String>().toList();

// in cascades
widget?..build(context)..render();
```

**Phase 1 Rules Requiring This:**
- `avoid-non-null-assertion` — detects use of `!` operator
- `no-boolean-literal-compare` — comparison patterns with null coalescing
- `no-equal-arguments` — null-aware argument comparison

**Priority:** CRITICAL

---

### 1.10 Patterns & Pattern Matching (Dart 3.0)

**Grammar Production:**
```
pattern
  : logical_and_pattern
  ;

logical_and_pattern
  : logical_or_pattern ('&&' logical_or_pattern)*
  ;

logical_or_pattern
  : relational_pattern ('||' relational_pattern)*
  ;

relational_pattern
  : cast_pattern
  | null_check_pattern
  | null_assert_pattern
  | constant_pattern
  | variable_pattern
  | identifier_pattern
  | list_pattern
  | map_pattern
  | record_pattern
  | object_pattern
  | parenthesized_pattern
  | wildcard_pattern
  ;

cast_pattern
  : primary_pattern 'as' type_reference
  ;

null_check_pattern
  : postfix_pattern '?'
  ;

null_assert_pattern
  : postfix_pattern '!'
  ;

constant_pattern
  : literal
  | identifier
  | 'const' primary_pattern
  ;

variable_pattern
  : ('var' | 'final' | type_reference)? identifier
  ;

identifier_pattern
  : 'const'? identifier
  ;

wildcard_pattern
  : '_'
  ;

list_pattern
  : '[' pattern_element? (',' pattern_element)* ','? ']'
  ;

pattern_element
  : pattern
  | rest_pattern
  ;

rest_pattern
  : '...' pattern?
  ;

map_pattern
  : '{' map_pattern_entry? (',' map_pattern_entry)* ','? '}'
  ;

map_pattern_entry
  : expression ':' pattern
  ;

record_pattern
  : '(' pattern (',' pattern)* ','? ')'
  ;

object_pattern
  : type_reference? '{' pattern_field? (',' pattern_field)* ','? '}'
  ;

pattern_field
  : identifier ':' pattern
  | identifier
  ;

parenthesized_pattern
  : '(' pattern ')'
  ;

switch_expression
  : 'switch' '(' expression ')'
    '{' switch_expression_case+ '}'
  ;

switch_expression_case
  : pattern guard_clause? '=>' expression ','?
  ;

guard_clause
  : 'when' expression
  ;
```

**Examples:**
```dart
// variable pattern
var (name, age) = ('Alice', 30);

// list pattern
var [first, second, ...rest] = [1, 2, 3, 4, 5];

// map pattern
var {'name': name, 'age': age} = data;

// record pattern (Dart 3.0)
var (name, :age, :email) = person;

// object pattern
var Point(x: x, y: y) = myPoint;

// wildcard pattern
var [_, second, _] = list;

// null-check pattern
if (value case int? x when x != null) {
  print(x);
}

// switch expression (Dart 3.0)
String describe(Object obj) => switch (obj) {
  int n when n < 0 => 'negative',
  int n when n == 0 => 'zero',
  int n => 'positive',
  String s => 'string: $s',
  [int first, ...] => 'list starting with $first',
  _ => 'unknown',
};

// pattern matching in switch statement (Dart 3.0)
switch (result) {
  case Success(data: var data):
    print('Success: $data');
  case Error(message: var msg):
    print('Error: $msg');
}

// guard clause with pattern
switch (command) {
  case ['help'] when verbose:
    showVerboseHelp();
  case ['help']:
    showHelp();
  case ['quit']:
    exit(0);
}

// cast pattern with null check
if (obj case String s when s.isNotEmpty) {
  print('Non-empty string: $s');
}
```

**Phase 1 Rules Requiring This:**
- All rules — pattern matching is fundamental to Dart 3.x code comprehension
- `no-equal-arguments` — pattern comparison
- `prefer-trailing-comma` — enforces trailing commas in pattern lists

**Priority:** CRITICAL

---

### 1.11 Records (Dart 3.0)

**Grammar Production:**
```
record_literal
  : '(' expression (',' expression)* ','? ')'
  | '(' expression ':' expression (',' expression ':' expression)* ','? ')'
  | '(' expression (',' expression)* ',' expression ':' expression (',' expression ':' expression)* ','? ')'
  ;
```

**Examples:**
```dart
// positional record
var pair = (1, 'hello');
var triple = (10, 'world', true);

// named record
var person = (name: 'Alice', age: 30);
var config = (host: 'localhost', port: 8080, debug: true);

// mixed record (positional and named)
var mixed = (42, 'info', enabled: true, timeout: 5000);

// record as function return
(String, int) getNameAndAge() => ('Bob', 25);

// record in type annotation
void processRecord((String, int) data) {
  var (name, age) = data;
  print('$name is $age years old');
}

// nested records
var nested = (
  user: (name: 'Charlie', id: 1),
  status: (active: true, verified: false)
);

// record in variable declaration
final (int x, int y) = computeCoordinates();
```

**Phase 1 Rules Requiring This:**
- `prefer-trailing-comma` — enforces trailing commas in record literals
- `no-equal-arguments` — detects duplicate fields in named records
- All rules — record pattern matching and destructuring

**Priority:** CRITICAL

---

### 1.12 Expressions: Operators, Cascade, Spread, Collection-if, Collection-for

**Grammar Production:**
```
expression
  : conditional_expression
  | throw_expression
  ;

conditional_expression
  : null_coalescing_expression
    ('?' expression ':' expression)?
  ;

null_coalescing_expression
  : logical_or_expression ('??' logical_or_expression)*
  ;

logical_or_expression
  : logical_and_expression ('||' logical_and_expression)*
  ;

logical_and_expression
  : equality_expression ('&&' equality_expression)*
  ;

equality_expression
  : relational_expression (('==' | '!=') relational_expression)*
  ;

relational_expression
  : additive_expression
    (('<' | '>' | '<=' | '>=') additive_expression)*
  | additive_expression 'is' type_reference
  | additive_expression 'is!' type_reference
  | additive_expression ('as') type_reference
  ;

additive_expression
  : multiplicative_expression (('+' | '-') multiplicative_expression)*
  ;

multiplicative_expression
  : unary_expression (('*' | '/' | '%' | '~/') unary_expression)*
  ;

unary_expression
  : ('+' | '-' | '~' | '!') unary_expression
  | postfix_expression
  ;

postfix_expression
  : primary_expression (postfix_operator)*
  | primary_expression ('++' | '--')
  | primary_expression (postfix_operator)* ('++' | '--')
  ;

postfix_operator
  : '.' identifier
  | '?.' identifier
  | '[' expression ']'
  | '(' argument_list? ')'
  | cascade_operator
  ;

cascade_operator
  : '..' (cascade_section)+
  | '?..' (cascade_section)+
  ;

cascade_section
  : '[' expression ']' postfix_operator*
  | identifier postfix_operator*
  | call_suffix
  ;

primary_expression
  : literal
  | identifier
  | 'this'
  | 'super' '.' identifier
  | constructor_invocation
  | function_expression
  | parenthesized_expression
  | list_literal
  | map_literal
  | set_literal
  | record_literal
  | throw_expression
  ;

list_literal
  : '[' list_element? (',' list_element)* ','? ']'
  ;

list_element
  : expression
  | spread_element
  | collection_if
  | collection_for
  ;

spread_element
  : '...' expression
  | '...?' expression
  ;

collection_if
  : 'if' '(' expression ')' list_element ('else' list_element)?
  ;

collection_for
  : 'for' '(' for_loop_parts ')' list_element
  ;

map_literal
  : '{' map_entry? (',' map_entry)* ','? '}'
  ;

map_entry
  : expression ':' expression
  | spread_element
  | collection_if
  | collection_for
  ;

set_literal
  : '{' set_element? (',' set_element)* ','? '}'
  ;

set_element
  : expression
  | spread_element
  | collection_if
  | collection_for
  ;
```

**Examples:**
```dart
// operators
int sum = 5 + 3;
bool equals = 5 == 5;
bool and = true && false;
bool or = true || false;
int result = 10 > 5 ? 20 : 30;
String text = condition ? 'yes' : 'no';

// cascade operator (..)
widget
  ..setColor(Colors.blue)
  ..setBorder(3.0)
  ..show();

// null-aware cascade (?..)
maybeWidget
  ?..setColor(Colors.blue)
  ?..show();

// spread operator (...)
List<int> list1 = [1, 2, 3];
List<int> list2 = [0, ...list1, 4];  // [0, 1, 2, 3, 4]

// null-coalescing spread (...?)
List<int>? maybeList;
List<int> combined = [0, ...?maybeList, 9];

// collection-if
List<String> items = [
  'item1',
  if (includeItem2) 'item2',
  'item3',
];

// collection-if with else
List<int> numbers = [
  1,
  if (isEven) 2 else 3,
  4,
];

// collection-for
List<int> doubled = [
  for (int i = 0; i < 5; i++)
    i * 2,
];

// map with collection-if and collection-for
Map<String, int> config = {
  'base': 10,
  if (isDev) 'debug': 1,
  for (String key in customKeys) key: values[key],
};

// set literal with spread
Set<int> combined = {1, 2, ...otherSet, 3};

// type assertions and casts
var obj = someObject;
(obj as String).toUpperCase();
if (obj is String) print(obj.length);
if (obj is! String) print('not a string');

// increment/decrement
int count = 5;
count++;
count--;
int next = ++count;
```

**Phase 1 Rules Requiring This:**
- All rules — expressions are fundamental
- `prefer-trailing-comma` — enforces trailing commas in literals and collections
- `no-equal-arguments` — detects duplicate arguments
- `avoid-non-null-assertion` — detects `as` with unnecessary assertions

**Priority:** CRITICAL

---

### 1.13 String Literals & Interpolation

**Grammar Production:**
```
string_literal
  : single_quoted_string
  | double_quoted_string
  | raw_string
  | multiline_string
  ;

single_quoted_string
  : '\'' string_content* '\''
  ;

double_quoted_string
  : '"' string_content* '"'
  ;

raw_string
  : 'r\'' string_content* '\''
  | 'r"' string_content* '"'
  ;

multiline_string
  : '\'\'\'' string_content* '\'\'\''
  | '"""' string_content* '"""'
  ;

string_content
  : character+
  | string_interpolation
  ;

string_interpolation
  : '$' identifier
  | '${' expression '}'
  ;
```

**Examples:**
```dart
// simple strings
String single = 'hello';
String double = "world";

// raw strings (backslashes literal)
String raw = r'C:\Users\Alice';
String rawDouble = r"Line 1\nLine 2";

// multiline strings
String multiline = '''
This is a
multiline string
with multiple lines.
''';

String tripleQuote = """
Also a multiline
string using triple quotes.
""";

// string interpolation
String name = 'Alice';
String greeting = 'Hello, $name!';

// complex interpolation
int count = 5;
String message = 'You have ${count > 1 ? count : count} item${count != 1 ? 's' : ''}.';

// nested interpolation in expressions
String status = 'Active: ${isActive ? "yes" : "no"}';

// interpolation with method calls
String upper = 'hello ${name.toUpperCase()}!';

// raw string with interpolation not processed
String literal = r'This $variable is not interpolated';

// escaped characters in strings
String escaped = 'Line 1\nLine 2\tTabbed';
```

**Phase 1 Rules Requiring This:**
- `format-comment` — checks string content in doc comments
- `double-literal-format` — enforces number format in strings
- All rules — string literal parsing

**Priority:** HIGH

---

### 1.14 Comments & Doc Comments

**Grammar Production:**
```
comment
  : single_line_comment
  | multi_line_comment
  | doc_comment
  ;

single_line_comment
  : '//' .*
  ;

multi_line_comment
  : '/*' .*? '*/'
  ;

doc_comment
  : '///' .*
  | '/**' .*? '*/'
  ;
```

**Examples:**
```dart
// This is a single-line comment

/*
This is a multi-line comment
spanning multiple lines.
*/

/// This is a doc comment for the function below.
/// It describes the function's behavior.
void myFunction() {}

/**
 * This is a multi-line doc comment.
 * It can span multiple lines.
 * [reference] to other members can be included.
 */
class MyClass {}

/// A doc comment with:
/// - Bullets
/// - And multiple items
///
/// And code examples:
/// ```dart
/// var x = 42;
/// ```
void anotherFunction() {}
```

**Phase 1 Rules Requiring This:**
- `format-comment` — enforces comment formatting
- `avoid-abbreviations-in-doc-comments` — flags abbreviated terms in docs
- `newline-before-return` — enforces spacing near comments

**Priority:** MEDIUM

---

### 1.15 Metadata & Annotations

**Grammar Production:**
```
metadata
  : '@' identifier type_arguments? ('(' argument_list ')')?
  ;

argument_list
  : argument (',' argument)*
  ;

argument
  : identifier? ':' expression
  | expression
  ;
```

**Examples:**
```dart
// simple annotation
@Deprecated('Use newFunction instead')
void oldFunction() {}

// annotation with arguments
@override
void method() {}

// custom annotations
@required
void processData(String data) {}

// annotation on class
@immutable
class User {
  final String name;
  User(this.name);
}

// annotation on variable
@visibleForTesting
int internalCounter = 0;

// multiple annotations
@Deprecated('v1')
@override
void legacy() {}

// annotation with named arguments
@Route(
  path: '/home',
  methods: ['GET', 'POST'],
)
void handleHome() {}

// conditional annotation (analysis only)
@if(kDebugMode)
void debugOnly() {}
```

**Phase 1 Rules Requiring This:**
- All rules — annotations affect rule application
- `correct-order-for-super-dispose` — respects `@override` annotation
- `avoid-unused-parameters` — respects `@visibleForTesting`

**Priority:** HIGH

---

### 1.16 Statements: Block, If, While, For, Do-While, Switch, Try-Catch, Return, Throw

**Grammar Production:**
```
statement
  : block_statement
  | if_statement
  | while_statement
  | do_while_statement
  | for_statement
  | switch_statement
  | try_statement
  | return_statement
  | throw_statement
  | break_statement
  | continue_statement
  | labeled_statement
  | expression_statement
  | variable_declaration_statement
  ;

block_statement
  : '{' statement* '}'
  ;

if_statement
  : 'if' '(' expression ')' statement ('else' statement)?
  ;

while_statement
  : 'while' '(' expression ')' statement
  ;

do_while_statement
  : 'do' statement 'while' '(' expression ')' ';'
  ;

for_statement
  : 'for' '(' for_loop_parts ')' statement
  | 'for' '(' identifier 'in' expression ')' statement
  | 'for' '(' type_reference identifier 'in' expression ')' statement
  ;

for_loop_parts
  : (variable_declaration | expression)? ';' expression? ';' expression*
  ;

switch_statement
  : 'switch' '(' expression ')' '{' switch_case* default_case? '}'
  ;

switch_case
  : 'case' expression ':' statement*
  ;

default_case
  : 'default' ':' statement*
  ;

try_statement
  : 'try' block_statement
    (catch_clause+ finally_clause? | finally_clause)
  ;

catch_clause
  : 'on' type_reference ('catch' '(' identifier (',' identifier)? ')')?
    block_statement
  ;

finally_clause
  : 'finally' block_statement
  ;

return_statement
  : 'return' expression? ';'
  ;

throw_statement
  : 'throw' expression ';'
  ;

break_statement
  : 'break' identifier? ';'
  ;

continue_statement
  : 'continue' identifier? ';'
  ;

labeled_statement
  : label ':' statement
  ;

expression_statement
  : expression ';'
  ;
```

**Examples:**
```dart
// block statement
{
  int x = 5;
  print(x);
}

// if statement
if (count > 0) {
  print('positive');
} else if (count < 0) {
  print('negative');
} else {
  print('zero');
}

// while loop
int i = 0;
while (i < 10) {
  print(i);
  i++;
}

// do-while loop
do {
  print('at least once');
} while (condition);

// for loop
for (int j = 0; j < 10; j++) {
  print(j);
}

// for-in loop
for (String item in items) {
  print(item);
}

// switch statement
switch (dayOfWeek) {
  case 1:
    print('Monday');
    break;
  case 2:
    print('Tuesday');
    break;
  default:
    print('Other day');
}

// try-catch-finally
try {
  riskyOperation();
} on SocketException catch (e) {
  print('Socket error: $e');
} on FormatException catch (e, stackTrace) {
  print('Format error: $e\n$stackTrace');
} catch (e) {
  print('Unknown error: $e');
} finally {
  cleanup();
}

// return statement
String getValue() {
  if (condition) return 'result';
  return 'default';
}

// throw statement
void validate(int age) {
  if (age < 0) {
    throw ArgumentError('Age must be non-negative');
  }
}

// labeled statement with break
outerLoop:
for (int i = 0; i < n; i++) {
  for (int j = 0; j < m; j++) {
    if (shouldBreak()) break outerLoop;
  }
}
```

**Phase 1 Rules Requiring This:**
- `newline-before-return` — enforces spacing before return statements
- `avoid-throw-in-catch-block` — detects throw in catch blocks
- `avoid-nested-if` — detects deeply nested if statements
- `max-switch-cases` — limits switch statement complexity
- `no-empty-block` — detects empty statement blocks

**Priority:** CRITICAL

---

## 2. Grammar Productions: OUT OF SCOPE for Phase 1

These constructs are either Dart 2.x-only (no longer relevant) or explicitly deferred to Phase 2. The parser MAY produce an error node for these or skip them without affecting Phase 1 rule accuracy.

### Out of Scope Constructs

| Construct | Reason | Phase 2 Plan |
|-----------|--------|-------------|
| `deferred` imports | Not required for any Phase 1 rule | Symbol resolution in Phase 2 |
| `external` function declarations | Interop-only; simplified to function stub | External call tracking in Phase 2 |
| `native` interop | Platform-specific; rare in jfit codebase | Native method resolution in Phase 2 |
| Full `const` expression evaluation | Semantic analysis beyond AST | Const evaluation engine in Phase 2 |
| Type inference & resolution | Cross-file symbol lookup | Type system in Phase 2 |
| Isolate & Zone APIs | Structural parsing only, no semantics | Concurrency analysis in Phase 2 |
| Operator overloading in const | Deferred semantic handling | Operator evaluation in Phase 2 |

### Simplified Handling

- **`deferred` imports**: Parse as regular import; diagnostics ignore `deferred` status
- **`external` declarations**: Parse as regular function/method; treat body as empty block
- **`native` declarations**: Parse as regular function; treat `native 'x'` as comment/annotation
- **Complex const expressions**: Parse as regular expression; mark as `UnknownConst` node type

---

## 3. AST Node Taxonomy

This section maps grammar productions to AST node types that the M1 parser must produce. These nodes align with `falcon_syntax::ast` type definitions that will be created in M1.3.

### Core AST Nodes (Enum Variants in Rust)

```rust
// Top-level AST
pub enum Program {
    SourceFile {
        library: Option<LibraryDirective>,
        imports: Vec<ImportDirective>,
        exports: Vec<ExportDirective>,
        declarations: Vec<Declaration>,
    },
}

pub enum Declaration {
    Class(ClassDeclaration),
    Mixin(MixinDeclaration),
    MixinClass(MixinClassDeclaration),
    Enum(EnumDeclaration),
    Extension(ExtensionDeclaration),
    ExtensionType(ExtensionTypeDeclaration),
    Function(FunctionDeclaration),
    Variable(VariableDeclaration),
    Error(ErrorNode),  // parse error recovery
}

pub struct ClassDeclaration {
    pub name: Identifier,
    pub modifiers: Vec<ClassModifier>,  // abstract, interface, base, final, sealed
    pub type_params: Vec<TypeParameter>,
    pub superclass: Option<TypeReference>,
    pub mixins: Vec<TypeReference>,
    pub interfaces: Vec<TypeReference>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

pub enum ClassMember {
    Constructor(ConstructorDeclaration),
    Method(MethodDeclaration),
    Field(FieldDeclaration),
    Getter(GetterDeclaration),
    Setter(SetterDeclaration),
    Error(ErrorNode),
}

pub struct MethodDeclaration {
    pub name: Identifier,
    pub modifiers: Vec<MethodModifier>,  // static, factory, override
    pub type_params: Vec<TypeParameter>,
    pub return_type: Option<TypeReference>,
    pub params: Vec<Parameter>,
    pub body: FunctionBody,
    pub span: Span,
}

pub struct FunctionDeclaration {
    pub name: Identifier,
    pub type_params: Vec<TypeParameter>,
    pub return_type: Option<TypeReference>,
    pub params: Vec<Parameter>,
    pub body: FunctionBody,
    pub span: Span,
}

pub enum FunctionBody {
    Block(Block),
    Arrow { expr: Box<Expression>, span: Span },
    Async(Box<Block>),
    AsyncStar(Box<Block>),
    SyncStar(Box<Block>),
    Error(ErrorNode),
}

pub enum Parameter {
    Positional {
        name: Identifier,
        type_: Option<TypeReference>,
        default: Option<Expression>,
        modifiers: Vec<ParameterModifier>,  // const, final, required, covariant
    },
    Named {
        name: Identifier,
        type_: Option<TypeReference>,
        default: Option<Expression>,
        required: bool,
        covariant: bool,
    },
    Error(ErrorNode),
}

pub struct VariableDeclaration {
    pub variables: Vec<VariableDeclarator>,
    pub modifiers: Vec<VarModifier>,  // final, const, late, var
    pub span: Span,
}

pub struct VariableDeclarator {
    pub name: Identifier,
    pub type_: Option<TypeReference>,
    pub initializer: Option<Expression>,
}

pub enum TypeReference {
    Named {
        name: Identifier,
        type_args: Vec<TypeReference>,
    },
    Nullable(Box<TypeReference>),
    Function(FunctionType),
    Record(RecordType),
    Void,
    Dynamic,
    Never,
    Object,
    Error(ErrorNode),
}

pub struct FunctionType {
    pub return_type: Box<TypeReference>,
    pub params: Vec<FunctionTypeParameter>,
    pub type_params: Vec<TypeParameter>,
    pub span: Span,
}

pub enum FunctionTypeParameter {
    Positional(TypeReference),
    Named { name: Identifier, type_: TypeReference },
}

pub struct RecordType {
    pub fields: Vec<RecordField>,
    pub span: Span,
}

pub enum RecordField {
    Positional(TypeReference),
    Named { name: Identifier, type_: TypeReference },
}

pub enum Statement {
    Block(Block),
    If(IfStatement),
    While(WhileStatement),
    DoWhile(DoWhileStatement),
    For(ForStatement),
    ForIn(ForInStatement),
    Switch(SwitchStatement),
    Try(TryStatement),
    Return(ReturnStatement),
    Throw(ThrowStatement),
    Break(Option<Identifier>),
    Continue(Option<Identifier>),
    Expression(Expression),
    Variable(VariableDeclaration),
    Labeled { label: Identifier, stmt: Box<Statement> },
    Error(ErrorNode),
}

pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

pub struct IfStatement {
    pub condition: Expression,
    pub then_stmt: Box<Statement>,
    pub else_stmt: Option<Box<Statement>>,
    pub span: Span,
}

pub struct SwitchStatement {
    pub expr: Expression,
    pub cases: Vec<SwitchCase>,
    pub span: Span,
}

pub struct SwitchCase {
    pub pattern: Pattern,
    pub guard: Option<Expression>,
    pub statements: Vec<Statement>,
    pub span: Span,
}

pub enum Pattern {
    Constant(Expression),
    Variable { name: Identifier, type_: Option<TypeReference> },
    Wildcard,
    List(ListPattern),
    Map(MapPattern),
    Record(RecordPattern),
    Object(ObjectPattern),
    Cast { pattern: Box<Pattern>, type_: TypeReference },
    NullCheck(Box<Pattern>),
    NullAssert(Box<Pattern>),
    LogicalAnd(Vec<Pattern>),
    LogicalOr(Vec<Pattern>),
    Error(ErrorNode),
}

pub struct ListPattern {
    pub elements: Vec<PatternElement>,
    pub span: Span,
}

pub enum PatternElement {
    Pattern(Pattern),
    Rest(Option<Box<Pattern>>),
}

pub enum Expression {
    Literal(Literal),
    Identifier(Identifier),
    Binary { left: Box<Expr>, op: BinaryOp, right: Box<Expr>, span: Span },
    Unary { op: UnaryOp, operand: Box<Expr>, span: Span },
    Assignment { lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
    Conditional { condition: Box<Expr>, then_expr: Box<Expr>, else_expr: Box<Expr>, span: Span },
    NullCoalescing { left: Box<Expr>, right: Box<Expr>, span: Span },
    Call { callee: Box<Expr>, args: Vec<Argument>, span: Span },
    Index { object: Box<Expr>, index: Box<Expr>, span: Span },
    Property { object: Box<Expr>, property: Identifier, span: Span },
    NullProperty { object: Box<Expr>, property: Identifier, span: Span },
    Cascade(Vec<CascadeSection>, Span),
    Cast { expr: Box<Expr>, type_: TypeReference, span: Span },
    Is { expr: Box<Expr>, type_: TypeReference, negated: bool, span: Span },
    As { expr: Box<Expr>, type_: TypeReference, span: Span },
    NonNullAssert(Box<Expr>, Span),
    List(ListLiteral),
    Map(MapLiteral),
    Set(SetLiteral),
    Record(RecordLiteral),
    Spread { expr: Box<Expr>, nullable: bool, span: Span },
    CollectionIf { condition: Box<Expr>, then_expr: Box<Expr>, else_expr: Option<Box<Expr>>, span: Span },
    CollectionFor { variable: Identifier, iterable: Box<Expr>, body: Box<Expr>, span: Span },
    StringInterpolation(Vec<StringPart>, Span),
    Function(FunctionExpression),
    Throw(Box<Expr>, Span),
    This(Span),
    Super(Span),
    Error(ErrorNode),
}

pub enum Literal {
    Integer(i64),
    Double(f64),
    String(String),
    Boolean(bool),
    Null,
}

pub struct Identifier {
    pub name: String,
    pub span: Span,
}

pub struct Span {
    pub start: usize,
    pub end: usize,
}

pub enum StringPart {
    Text(String),
    Interpolation { expr: Expression, span: Span },
}

pub struct ErrorNode {
    pub message: String,
    pub span: Span,
}
```

### Phase 1 Rule → AST Node Mapping

| Phase 1 Rule | Primary AST Nodes Used |
|---|---|
| `avoid-dynamic` | `TypeReference::Dynamic`, `Declaration::Variable` |
| `avoid-global-state` | `Declaration::Variable` (top-level) |
| `avoid-late-keyword` | `VariableDeclaration` (with `late` modifier) |
| `avoid-nested-conditional-expressions` | `Expression::Conditional` (nested) |
| `avoid-non-null-assertion` | `Expression::NonNullAssert` |
| `avoid-throw-in-catch-block` | `TryStatement::catch_clause`, `Statement::Throw` |
| `avoid-unused-parameters` | `Parameter` nodes in `FunctionDeclaration`, `MethodDeclaration` |
| `double-literal-format` | `Literal::Double` |
| `member-ordering` | `ClassDeclaration::members` |
| `no-boolean-literal-compare` | `Literal::Boolean`, `Expression::Binary` |
| `no-empty-block` | `Block` with zero statements |
| `no-equal-arguments` | `Expression::Call` with duplicate arguments |
| `no-equal-then-else` | `IfStatement` with identical branches |
| `newline-before-return` | `Statement::Return`, `Span` (line tracking) |
| `prefer-async-await` | `FunctionBody::AsyncStar` (detects generators) |
| `prefer-conditional-expressions` | `IfStatement` in expression context |
| `prefer-first` | `Expression::Call` on iterables |
| `prefer-immediate-return` | `Statement::Return` in function body |
| `prefer-last` | `Expression::Call` on iterables |
| `prefer-trailing-comma` | `ListLiteral`, `MapLiteral`, `RecordLiteral`, `Parameter` (span tracking) |
| `format-comment` | Comment tokens from lexer |
| `no-magic-number` | `Literal::Integer`, `Literal::Double` |
| `max-lines-for-function` | `FunctionDeclaration`, `Span` |
| ... | (all 60 rules use various AST nodes) |

---

## 4. Edge Cases to Handle

The parser must handle these tricky productions correctly to avoid cascading errors in rule analysis.

### 4.1 Deeply Nested Type Parameters

**Challenge:** No maximum depth; AST must support arbitrary nesting.

```dart
// Extremely nested type
Map<String, List<Map<String, List<Set<dynamic>>>>> nested;

// Function type with nested generics
Future<Result<Data<List<Map<String, dynamic>>>>> asyncFetch();

// Record with nested generics
({List<Map<String, int>> data, Set<String> tags}) config;
```

**Solution:**
- Recursive `TypeReference` enum (allows Box for indirection)
- Stack-based parser to avoid stack overflow on very deep nesting
- Test with 10+ levels of nesting

### 4.2 Complex Function Types

**Challenge:** Function types can have positional, named, optional parameters; return types; and type parameters.

```dart
// Function type with all parameter kinds
void Function(
  String required,
  int? optional,
  {required bool flag, String? name, Duration timeout = Duration(seconds: 5)}
)? nullable;

// Nested function type
Future<void Function({required String input})> createHandler();
```

**Solution:**
- Separate `FunctionType` AST node with dedicated parameter parsing
- Support all parameter modifiers (required, covariant, final, etc.)
- Test with 10+ parameter combinations

### 4.3 Record Types & Literals

**Challenge:** Records can mix positional and named fields; trailing commas are significant.

```dart
// Positional record
var p1 = (1, 2, 3);

// Named record
var p2 = (x: 1, y: 2);

// Mixed record (positional first, then named)
var p3 = (1, 2, x: 3, y: 4);

// Record type with trailing comma
({int x, String y,}) recordType;

// Destructuring with records
var (a, :b, :c) = someRecord;
```

**Solution:**
- Separate `RecordLiteral` and `RecordType` AST nodes
- Track positional vs named fields separately
- Preserve trailing comma information (for `prefer-trailing-comma` rule)
- Test with 5+ field combinations

### 4.4 Pattern Matching (Dart 3.0)

**Challenge:** Patterns can nest arbitrarily; guard clauses affect control flow.

```dart
// Complex nested pattern
switch (data) {
  case [int first, ...List<String> rest] when first > 0:
    print('Valid list');
  case (:int x, :int y) when x + y > 100:
    print('Large sum');
  case {"status": String status, "code": int code}:
    print('Map pattern');
}

// Pattern in variable declaration
var [first, ...rest] = list;

// Nested record pattern
var ((:x, :y), :z) = nestedRecord;
```

**Solution:**
- Recursive `Pattern` enum supporting all pattern types
- Separate `guard_clause` parsing in switch cases
- Stack-based parser for nested patterns
- Test with 5+ nesting levels and all pattern types

### 4.5 String Interpolation with Expressions

**Challenge:** Interpolations can contain complex expressions with braces; must track nesting.

```dart
// Simple interpolation
var s1 = 'Hello, $name!';

// Expression interpolation
var s2 = 'Result: ${obj.method(arg)}';

// Nested braces in interpolation
var s3 = 'List: ${items.map((x) => x.toString()).join(", ")}';

// Interpolation with collections
var s4 = 'Data: ${{"key": value, "nested": [1, 2, 3]}}';

// Raw string (interpolation disabled)
var s5 = r'This $var is not interpolated';
```

**Solution:**
- Track brace depth during string parsing
- Switch to expression parser when entering `${...}`
- Emit `StringPart::Text` and `StringPart::Interpolation` nodes
- Test with 3+ levels of brace nesting

### 4.6 Cascade with Null-Aware Operators

**Challenge:** Cascades can be null-aware and chain multiple operations.

```dart
// Simple cascade
widget
  ..setColor(Colors.blue)
  ..setBorder(3.0)
  ..show();

// Null-aware cascade
widget
  ?..setColor(Colors.blue)
  ?..show();

// Cascade with subscript and property access
obj
  ..[0].property = value
  ..["key"] = value2
  ..method();
```

**Solution:**
- Dedicated `Expression::Cascade` node
- Track null-aware prefix (`?.`)
- Parse cascade sections as chained property/index accesses
- Test with 5+ cascade operations and null-awareness

### 4.7 Trailing Commas (Everywhere)

**Challenge:** Trailing commas are allowed in most collections and parameter lists; rules check for consistency.

```dart
// List with trailing comma
List<int> items = [
  1,
  2,
  3,  // <-- trailing comma
];

// Function parameters with trailing comma
void function(
  String a,
  int b,
  {required bool c,}  // <-- trailing comma
) {}

// Record with trailing comma
var record = (
  x: 1,
  y: 2,
);

// No trailing comma
List<int> compact = [1, 2, 3];

// Single-line with comma
var pair = (1, 2,);
```

**Solution:**
- Preserve trailing comma information in AST nodes
- Add `has_trailing_comma: bool` field to collection literals and parameter lists
- Test with all collection types and parameter configurations

### 4.8 Comments & Doc Comments

**Challenge:** Comments must be preserved for rules like `format-comment`; doc comments affect class/method documentation.

```dart
// Single-line comment
int x = 5;  // comment

/// Doc comment
/// Multiple lines
/// [reference] to methods
void function() {
  /*
   * Multi-line comment
   */
  operation();
}

// Comment after comma
List<int> items = [
  1,  // first
  2,  // second
];
```

**Solution:**
- Lex comments as tokens (include them in token stream)
- Parser associates comments with nearby AST nodes (optional)
- Test with comments in all positions (before, after, inside)

---

## 5. Error Recovery Strategy

The parser must recover gracefully from syntax errors and continue parsing to provide diagnostics for the entire file.

### 5.1 Error Handling Principles

1. **Emit ErrorNode instead of panicking**: When a production cannot be parsed, emit an `ErrorNode` with message and span.
2. **Skip to recovery point**: Resume parsing at a predictable boundary (e.g., next statement, next top-level declaration).
3. **Continue file processing**: Parse rest of file to collect all diagnostics (not just the first error).
4. **Suppress cascades**: Rules visiting `ErrorNode` should suppress diagnostics to avoid spurious violations.

### 5.2 Recovery Boundaries

| Context | Recovery Boundary |
|---------|-------------------|
| Statement parsing error | Skip to next `;` or `}` (end of block) |
| Expression parsing error | Skip to next statement-ending token (`;`, `)`, `}`, `,`) |
| Class member parsing error | Skip to next member declaration or `}` |
| Parameter parsing error | Skip to next `,` or `)` |
| Type parsing error | Skip to next structural keyword or `;` |

### 5.3 Error Node Types

```rust
pub struct ErrorNode {
    pub message: String,
    pub span: Span,
    pub recovery_text: String,  // tokens consumed before recovery
}
```

### 5.4 Examples of Error Recovery

**Syntax Error 1: Missing semicolon**
```dart
int x = 5  // <-- missing semicolon
String name = "Alice";
```
**Recovery:** Emit error at end of `5` span; treat `String` as start of new statement.

**Syntax Error 2: Invalid type parameter**
```dart
class Container<T extends SomethingInvalid> {  // <-- invalid base type
  T item;
}
```
**Recovery:** Emit error; treat `SomethingInvalid` as unresolved type; continue parsing class body.

**Syntax Error 3: Malformed expression**
```dart
int result = 5 + * 3;  // <-- invalid operator sequence
```
**Recovery:** Emit error at `*`; skip to `;`; continue parsing.

---

## 6. Phase 2 Deferrals

These grammar constructs will be added in Phase 2 (if at all). Phase 1 parser must NOT attempt to implement these.

### 6.1 Full Const Evaluation Context

**Deferred rule:** `no-magic-number` — Phase 1 uses simplified heuristic (ban all numeric literals except 0, 1, -1); Phase 2 evaluates const expressions.

**Why deferred:**
- Requires semantic analysis (scope lookup, type resolution)
- Parser alone cannot evaluate `const duration = Duration(days: 30)` as non-magic
- Phase 2 will add const evaluation engine

### 6.2 Type Inference & Type Resolution

**Deferred rules:**
- `avoid-unnecessary-type-assertions`
- `avoid-unnecessary-type-casts`
- `avoid-unrelated-type-assertions`
- `prefer-iterable-of`
- `unnecessary-nullable-return-type`

**Why deferred:**
- Requires full type system implementation
- Parser cannot determine if `as String` is unnecessary without type resolution
- Phase 2 will add type resolver

### 6.3 Cross-File Symbol Resolution

**Deferred rules:**
- `avoid-returning-widgets`
- `avoid-passing-async-when-sync-expected`
- `unnecessary-flutter-imports`
- `use-once-constructors-once-provider`

**Why deferred:**
- Requires scope lookup across files
- Parser cannot know if `String` imported from `package:my_lib` is a widget
- Phase 2 will add symbol table and cross-file analysis

### 6.4 Operator Overloading in Const Expressions

**Deferred feature:** Full const evaluation with operator overload resolution.

**Why deferred:**
- Semantic analysis beyond parser scope
- Phase 1 treats `const x = 5 + 3 * 2` as constant value (not evaluated)
- Phase 2 will evaluate operator precedence

---

## 7. Testing Strategy for M1 Parser

### 7.1 Lexer Tests (M1.1)

- **Unit tests**: Tokenize each keyword, operator, string variant (50+ test cases)
- **Integration tests**: Lex real jfit .dart files (5+ files from corpus)
- **Snapshot tests**: Token stream output for complex code

### 7.2 Parser Tests (M1.2)

- **Production tests**: 3-5 test cases per major production (100+ tests)
- **Edge case tests**: Deeply nested types, complex function types, pattern matching
- **Error recovery tests**: Malformed input produces error nodes, not panics
- **Corpus tests**: Parse all 214 jfit .dart files without panic

### 7.3 Integration Tests (M1.4)

- **Full corpus**: Parse entire jfit mobile lib; snapshot AST
- **Round-trip (optional)**: Parse → serialize AST → validate structure
- **Regression**: Any parser change requires snapshot review

### 7.4 Performance Baseline (M1.5)

- **Benchmark**: Parse 50-file jfit sample; measure time
- **Target**: <100ms single-threaded baseline
- **Acceptance**: Performance locked at M1.5; future changes justified

---

## 8. Parsing Algorithm Recommendations

### 8.1 Top-Down Recursive Descent

**Recommended approach for falcon:**
- Straightforward implementation
- Good error recovery properties
- Suitable for Dart grammar (mostly unambiguous, minimal backtracking)

**Parser structure:**
```rust
struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn parse_program(&mut self) -> Result<Program, ErrorNode> { ... }
    fn parse_class_declaration(&mut self) -> Result<ClassDeclaration, ErrorNode> { ... }
    fn parse_function(&mut self) -> Result<FunctionDeclaration, ErrorNode> { ... }
    fn parse_expression(&mut self) -> Result<Expression, ErrorNode> { ... }
    fn parse_type(&mut self) -> Result<TypeReference, ErrorNode> { ... }
    fn parse_pattern(&mut self) -> Result<Pattern, ErrorNode> { ... }
}
```

### 8.2 Expression Parsing with Precedence Climbing

**For binary operators:**
```rust
fn parse_binary_expression(&mut self, min_prec: i32) -> Result<Expression, ErrorNode> {
    let mut left = self.parse_primary()?;
    
    while self.peek_token().precedence() >= min_prec {
        let op = self.next_token();
        let right = self.parse_binary_expression(op.precedence() + 1)?;
        left = Expression::Binary { left, op, right, span };
    }
    
    Ok(left)
}
```

### 8.3 Error Recovery with Synchronization Points

**Strategy:**
```rust
fn skip_to_recovery_point(&mut self, context: &str) {
    // Skip tokens until reaching a recovery boundary
    // (e.g., `;`, `}`, `class`, `void`)
    loop {
        match self.peek_token().kind {
            TokenKind::Semicolon | TokenKind::RightBrace => break,
            TokenKind::Keyword("class" | "void" | ...) => break,
            TokenKind::Eof => break,
            _ => { self.next_token(); }
        }
    }
}
```

---

## 9. Summary: What M1 Parser Must Deliver

### 9.1 Lexer Output

- Complete token stream for all Dart 3.x tokens
- Correct handling of strings, comments, operators
- Error tokens for malformed input (not panics)

### 9.2 Parser Output

- Full AST for all Phase 1 grammar productions (sections 1.1-1.16)
- Error recovery: malformed code produces `ErrorNode`, not panic
- Span tracking: every node has byte-accurate source location

### 9.3 AST Structure

- Matches taxonomy in section 3
- Supports all Phase 1 rule requirements
- Extensible for Phase 2 without breaking changes

### 9.4 Quality Metrics (M1.5)

- 100% of jfit mobile lib (214 files) parses without panic
- 100+ parser unit tests pass
- <100ms single-threaded parse time (50-file baseline)
- All Phase 1 rules can analyze resulting AST without parser changes

---

## 10. References & Specification Sources

- **Dart Language Specification**: https://dart.dev/guides/language/spec
- **Dart 3.0 Release Notes**: https://dart.dev/guides/whats-new/release-notes/release-notes-3.0
- **Dart 3.x Enhancement Proposals**: https://github.com/dart-lang/language/issues
- **dart_code_linter Source**: https://github.com/CodingMatterlab/dart-code-linter
- **pyramid_lint Source**: https://github.com/peterdewinter/pyramid_lint
- **jfit Analysis Options**: `jfit/analysis_options.yaml` (reference configuration)

---

## Appendix: Quick Reference — Grammar Tokens

| Category | Examples |
|----------|----------|
| **Keywords** | `class`, `void`, `async`, `await`, `yield`, `final`, `const`, `var`, `late`, `required`, `covariant`, `extends`, `implements`, `with`, `on`, `as`, `is`, `super`, `this`, `new`, `switch`, `case`, `default`, `if`, `else`, `for`, `while`, `do`, `try`, `catch`, `finally`, `throw`, `return`, `break`, `continue`, `import`, `export`, `library`, `part`, `deferred`, `static`, `abstract`, `interface`, `sealed`, `base`, `final`, `mixin` |
| **Operators** | `+`, `-`, `*`, `/`, `%`, `~/`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `\|\|`, `!`, `&`, `\|`, `^`, `<<`, `>>`, `~`, `?.`, `??`, `??=`, `..`, `...`, `...?`, `=>`, `++`, `--` |
| **Punctuation** | `(`, `)`, `[`, `]`, `{`, `}`, `,`, `;`, `:`, `.`, `@` |
| **Literals** | integers, doubles, strings (single/double/raw/multiline), booleans, null |
| **Identifiers** | user-defined names, class names, function names |

---

**Document Status:** LOCKED FOR M1  
**Last Updated:** 2026-06-09  
**Next Review:** After M1.5 AST format specification
