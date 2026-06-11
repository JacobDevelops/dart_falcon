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

  void processWithValidation(List<String> items) {
    items.forEach((item) { /* expect: prefer-extracting-callbacks */
      final trimmed = item.trim();
      final lowercased = trimmed.toLowerCase();
      if (lowercased.isNotEmpty) {
        print(lowercased);
      }
    });
  }

  List<int> filterNumbers(List<int> nums) {
    return nums.where((num) { /* expect: prefer-extracting-callbacks */
      final doubled = num * 2;
      final squared = doubled * doubled;
      final final_val = squared + num;
      return final_val > 100;
    }).toList();
  }

  void setupHandlers(List<String> events) {
    events.forEach((event) { /* expect: prefer-extracting-callbacks */
      final parts = event.split(':');
      final key = parts[0];
      final value = parts.length > 1 ? parts[1] : '';
      _handle(key, value);
    });
  }

  void _handle(String key, String value) {}
}
