class Token {
  const Token._(this.value);
  final int value;

  static const int seed = 1;
  static const int next = seed + 1;
  static const Token first = Token._(seed);
  static const Token firstAlias = Token._(1);
  static const Token second = Token._(next);
}

void missingDependencyValue(Token token) {
  switch (token) { /* expect: exhaustive-cases */
    case Token.firstAlias:
      break;
  }
}

void allDependencyValues(Token token) {
  switch (token) {
    case Token.first:
      break;
    case Token.second:
      break;
  }
}
