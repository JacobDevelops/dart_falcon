// @dart = 2.19
// Before Dart 3.0 a case falls through without its trailing `break`.
void route(int code) {
  switch (code) {
    case 1:
      print('one');
      break;
    default:
      print('other');
  }
}
