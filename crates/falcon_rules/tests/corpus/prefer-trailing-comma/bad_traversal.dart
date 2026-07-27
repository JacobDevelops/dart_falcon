class Mark {
  const Mark(String value);
}

@Mark(
  'annotation'
) /* expect: prefer-trailing-comma */
class Base {
  Base(String value);
}

class Child extends Base {
  Child()
      : super(
          'initializer'
        ); /* expect: prefer-trailing-comma */
}

enum Choice {
  first(
    'enum'
  ); /* expect: prefer-trailing-comma */

  const Choice(String value);
}

void cascade(dynamic target) {
  target..call(
    'cascade'
  ); /* expect: prefer-trailing-comma */
}
