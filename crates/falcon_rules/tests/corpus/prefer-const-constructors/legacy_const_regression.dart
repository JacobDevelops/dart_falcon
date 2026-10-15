// @dart=2.19
class Status {
  final int code;
  const Status(this.code);
}
int dispatch(Status status) {
  switch (status) {
    case Status(200): return 1;
    case Status(404): return 2;
    default: return 0;
  }
}
final ordinary = Status(500); /* expect: prefer-const-constructors */
