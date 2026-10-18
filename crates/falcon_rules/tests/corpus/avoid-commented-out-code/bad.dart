void example() {
  /* expect: avoid-commented-out-code */ // print('old');
  /* expect: avoid-commented-out-code */ // final count = 3;
  /* expect: avoid-commented-out-code */ // value = false;
  /* expect: avoid-commented-out-code */ // return null;
  /* throw StateError('obsolete'); */ /* expect: avoid-commented-out-code */
  /* expect: avoid-commented-out-code */ // service.refresh();
  /* expect: avoid-commented-out-code */ // final int count = 1;
  /* expect: avoid-commented-out-code */ // return 42;
}
/* expect: avoid-commented-out-code */ // void obsolete(String value) {
// }
/* expect: avoid-commented-out-code */ // class Obsolete {
// }
/* expect: avoid-commented-out-code */ /*
 * print('old block');
 */
