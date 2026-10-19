void f(bool flag, dynamic value) {
  if (flag) {} else {}
  if (!flag) return;
  if (value == 1) {} else {}
  if (value is String) {} else {}
  final a = flag ? 1 : 2;
  if (!flag) {} else if (value == 1) {}
  final list = [if (flag) 1 else 2];
  final set = {if (!flag) 1};
  final map = {if (flag) 1: 2 else 3: 4};
  final mapWithoutElse = {if (value != 1) 1: 2};
  final chained = [if (!flag) 1 else if (value == 1) 2];
  final chainedMap = {if (!flag) 1: 2 else if (value == 1) 3: 4};
}
