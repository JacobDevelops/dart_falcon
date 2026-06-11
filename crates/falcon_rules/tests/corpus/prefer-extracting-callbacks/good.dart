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

// OK: simple inline callbacks (one-liners or arrow functions)
class Simple {
  List<int> doubleValues(List<int> nums) {
    return nums.map((n) => n * 2).toList();
  }

  void printIfEven(List<int> nums) {
    nums.where((n) => n % 2 == 0).forEach(print);
  }
}

// Good: extracted callback method for validation
class Validator {
  void _logProcessing(String item) {
    final trimmed = item.trim();
    final lowercased = trimmed.toLowerCase();
    if (lowercased.isNotEmpty) {
      print(lowercased);
    }
  }

  void processWithValidation(List<String> items) {
    items.forEach(_logProcessing);
  }
}

// Good: single-statement callbacks are fine
class Calculator {
  List<int> transform(List<int> values) {
    return values.map((n) => n * 2 + 1).toList();
  }

  void process(List<String> data) {
    data.forEach(print);
  }
}

// Good: extracting filter logic
class Filter {
  bool _isSignificant(int num) {
    final doubled = num * 2;
    final squared = doubled * doubled;
    final final_val = squared + num;
    return final_val > 100;
  }

  List<int> filterNumbers(List<int> nums) {
    return nums.where(_isSignificant).toList();
  }
}
