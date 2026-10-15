import 'member_eligibility.dart' as p;

int parse(String value) => 0;

class Parsers {
  static int parse(String value) => 0;
  static const parser = int.parse;
  static final runtimeParser = int.parse;
}

class Holder {
  final Object value;
  const Holder(this.value);
}

class ImplicitConstructor {}

void constants() {
  Holder(Parsers.parse); /* expect: prefer-const-constructors */
  Holder(Parsers.parser); /* expect: prefer-const-constructors */
  Holder(p.parse); /* expect: prefer-const-constructors */
  Holder(ImplicitConstructor.new); /* expect: prefer-const-constructors */
  Holder(Parsers.runtimeParser);
}
