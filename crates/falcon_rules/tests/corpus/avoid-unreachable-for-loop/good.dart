void valid(List<int> items, List<int> others) {
  for (final item in items) { print(item); }
  if (items.isNotEmpty) { items.forEach(print); }
  if (items.length != 0) { for (final item in items) { print(item); } }
  if (items.isEmpty) { for (final item in others) { print(item); } }
  // Mutation between guard and loop invalidates the empty fact.
  if (items.isEmpty) { items.add(1); for (final item in items) { print(item); } }
  if (items.isEmpty) { final items = others; items.forEach(print); }
  if (items.isEmpty) { print('empty'); } else { items.forEach(print); }
}
class Custom {
  bool get isEmpty => true;
  void forEach(void Function(int) action) { action(1); }
}
void custom(Custom items) {
  if (items.isEmpty) { items.forEach(print); }
}
