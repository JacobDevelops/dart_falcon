enum Color { red, green, blue }

Color pick() => Color.red;

class Status {
  final int code;
  const Status._(this.code);
  static const ok = Status._(0);
  static const failed = Status._(1);
}

void f(Color color, Status status, Color? maybe) {
  switch (color) {
    case Color.red:
      print('red');
    default: /* expect: no-default-cases */
      print('other');
  }
  switch (status) {
    case Status.ok:
      print('ok');
    default: /* expect: no-default-cases */
      print('failed');
  }
  switch (color) {
    case Color.green:
    default: /* expect: no-default-cases */
      print('green or other');
  }
  switch (maybe) {
    case Color.blue:
      print('blue');
    default: /* expect: no-default-cases */
      print('other');
  }
  switch (pick()) {
    case Color.red:
      break;
    default: /* expect: no-default-cases */
      break;
  }
}
