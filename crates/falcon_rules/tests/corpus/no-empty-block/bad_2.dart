// Additional no-empty-block coverage: the member/declaration kinds folded in
// from the former pyramid_lint twins — operators, extensions, enums, and
// switch-case bodies. Diagnostics report at the closing brace.

class Vector {
  Vector operator +(Vector other) {
  } /* expect: no-empty-block */
}

extension StringX on String {
  void shout() {
  } /* expect: no-empty-block */
}

enum Color {
  red,
  green;

  void describe() {
  } /* expect: no-empty-block */
}

void handle(int code) {
  switch (code) {
    case 1:
      {
      } /* expect: no-empty-block */
      break;
  }
}
