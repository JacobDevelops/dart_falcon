class Deprecated {
  const Deprecated(String message);
}

class Token {
  final int value;
  const Token._(this.value);
  static const first = Token._(0);
  @Deprecated('not an SDK annotation')
  static const annotated = Token._(1);
  static const duplicate = Token._(1);
}

void inspect(Token token) {
  switch (token) { /* expect: exhaustive-cases, msg: "Add a case for `Token.annotated`." */
    case Token.first:
      break;
  }
}
