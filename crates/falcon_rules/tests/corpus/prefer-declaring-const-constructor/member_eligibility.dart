import 'member_eligibility.dart' as p;

int parse(String value) => 0;

class Parsers {
  static int parse(String value) => 0;
  static const parser = int.parse;
  static final runtimeParser = int.parse;
}

class StaticHolder {
  final parser = Parsers.parse;
  StaticHolder(); /* expect: prefer-declaring-const-constructor */
}

class PrefixedHolder {
  final parser = p.parse;
  PrefixedHolder(); /* expect: prefer-declaring-const-constructor */
}

class ConstantHolder {
  final parser = Parsers.parser;
  ConstantHolder(); /* expect: prefer-declaring-const-constructor */
}

class RuntimeHolder {
  final parser = Parsers.runtimeParser;
  RuntimeHolder();
}
