class Theme {
  static final padding = 1;
  static const fixed = 1;
}

// A constant type-literal receiver does not make a static-final read constant.
class Direct {
  final value = Theme.padding;
  Direct();
}

class Initializer {
  final int value;
  Initializer() : value = Theme.padding;
}

class Fixed {
  final value = Theme.fixed;
  Fixed(); /* expect: prefer-declaring-const-constructor */
}
