// Bad: inline callback with complex logic
class DataProcessor {
  List<String> processItems(List<Map<String, dynamic>> items) {
    return items.map((item) { /* expect: prefer-extracting-callbacks */
      final id = item['id'] as String;
      final name = item['name'] as String;
      final active = item['active'] as bool;
      if (active) {
        return '$id: $name';
      }
      return '';
    }).toList();
  }

  void filterAndPrint(List<int> numbers) {
    numbers.where((n) { /* expect: prefer-extracting-callbacks */
      final isEven = n % 2 == 0;
      final isPositive = n > 0;
      final isSmall = n < 100;
      return isEven && isPositive && isSmall;
    }).forEach(print);
  }
}
