void text(String? checked, String replacement) {
  if (checked != null) { print(replacement); } /* expect: avoid-unused-after-null-check */
}
void integer(int? checked, int replacement) {
  if (null != checked) { print(replacement); } /* expect: avoid-unused-after-null-check */
}
void decimal(double? checked, double replacement) {
  if ((checked) != null) { print(replacement); } /* expect: avoid-unused-after-null-check */
}
void boolean(bool? checked, bool replacement) {
  if (checked != null) { print(replacement); } /* expect: avoid-unused-after-null-check */
}
void consume({required String value}) {}
void named(String? checked, String replacement) {
  if (checked != null) { consume(value: replacement); } /* expect: avoid-unused-after-null-check */
}
void shadowed(String? checked, String replacement) {
  if (checked != null) { { final checked = 'other'; print(checked); } print(replacement); } /* expect: avoid-unused-after-null-check */
}
void headline(String value) {
  final String? myNullableValue = 'hello';
  if (myNullableValue != null) { print(value); } /* expect: avoid-unused-after-null-check */
}
class Item {}
void userType(Item? checked, Item replacement) {
  if (checked != null) { print(replacement); } /* expect: avoid-unused-after-null-check */
}
void numeric(num? checked, num replacement) {
  if (checked != null) { print(replacement); } /* expect: avoid-unused-after-null-check */
}
void matchingGeneric(List<int>? checked, List<int> replacement) {
  if (checked != null) { print(replacement); } /* expect: avoid-unused-after-null-check */
}
