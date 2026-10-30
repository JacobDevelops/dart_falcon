void check(int value, Object? item) {
  if (value == 3 &&
      value != 4) {} /* expect: avoid-excessive-expressions */
  if (5 < value &&
      value > 4) {} /* expect: avoid-excessive-expressions */
  if (value <= 3 ||
      value < 2) {} /* expect: avoid-excessive-expressions */
  if (value >= -2 &&
      -3 <= value) {} /* expect: avoid-excessive-expressions */
  if (item != null && /* expect: avoid-excessive-expressions */
      item is int) {}
  if (item == null || /* expect: avoid-excessive-expressions */
      item is! int) {}
  if (item is String &&
      null != item) {} /* expect: avoid-excessive-expressions */
  if (value > 4 && /* expect: avoid-excessive-expressions */
      value > 5) {}
  if (value < 2 || /* expect: avoid-excessive-expressions */
      value <= 3) {}
  if (value > -(-3) && /* expect: avoid-excessive-expressions */
      value > 4) {}
}
