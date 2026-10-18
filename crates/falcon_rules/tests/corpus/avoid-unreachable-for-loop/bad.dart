void listLoop(List<int> items) {
  if (items.isEmpty) { for (final item in items) { print(item); } } /* expect: avoid-unreachable-for-loop */
}
void listCallback(List<int> items) {
  if (items.length == 0) { items.forEach(print); } /* expect: avoid-unreachable-for-loop */
}
void reversedLength(List<int> items) {
  if (0 == items.length) { for (final item in items) { print(item); } } /* expect: avoid-unreachable-for-loop */
}
void negated(List<int> items) {
  if (!items.isNotEmpty) { items.forEach(print); } /* expect: avoid-unreachable-for-loop */
}
void setLoop(Set<String> items) {
  if (items.isEmpty) { for (final item in items) { print(item); } } /* expect: avoid-unreachable-for-loop */
}
void mapCallback(Map<String, int> items) {
  if (items.isEmpty) { items.forEach((key, value) { print(value); }); } /* expect: avoid-unreachable-for-loop */
}
