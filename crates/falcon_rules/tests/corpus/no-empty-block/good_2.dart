// Non-empty counterparts of bad_2.dart — none must fire. The switch case holds
// a comment-only block, which is treated as intentional (not flagged).

class Vector {
  Vector operator +(Vector other) {
    return other;
  }
}

extension StringX on String {
  void shout() {
    print(toUpperCase());
  }
}

enum Color {
  red,
  green;

  void describe() {
    print(name);
  }
}

void handle(int code) {
  switch (code) {
    case 1:
      {
        // intentionally does nothing here
      }
      break;
  }
}
