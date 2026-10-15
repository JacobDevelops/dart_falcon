class Token {
  final int value;
  const Token._(this.value);
  static const first = Token._(0);
  @Deprecated('use second')
  static const oldSecond = Token._(1);
  static const second = Token._(1);
  static const third = Token._(2);
  static const fourth = Token._(3);
}
void inspect(Token token) {
  switch (token) { /* expect: exhaustive-cases */ /* expect: exhaustive-cases */ /* expect: exhaustive-cases */
    case Token.first:
      break;
  }
}
