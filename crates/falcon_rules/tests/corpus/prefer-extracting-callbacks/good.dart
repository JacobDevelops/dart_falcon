// Good: extracting complex callbacks to named functions
class DataProcessor {
  String _formatItem(Map<String, dynamic> item) {
    final id = item['id'] as String;
    final name = item['name'] as String;
    final active = item['active'] as bool;
    if (active) {
      return '$id: $name';
    }
    return '';
  }

  List<String> processItems(List<Map<String, dynamic>> items) {
    return items.map(_formatItem).toList();
  }

  bool _isValidNumber(int n) {
    final isEven = n % 2 == 0;
    final isPositive = n > 0;
    final isSmall = n < 100;
    return isEven && isPositive && isSmall;
  }

  void filterAndPrint(List<int> numbers) {
    numbers.where(_isValidNumber).forEach(print);
  }
}

// OK: simple inline callbacks
class Simple {
  List<int> doubleValues(List<int> nums) {
    return nums.map((n) => n * 2).toList();
  }

  void printIfEven(List<int> nums) {
    nums.where((n) => n % 2 == 0).forEach(print);
  }
}
