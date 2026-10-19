void f(bool flag, dynamic value) {
  if (!flag) {} else {} /* expect: avoid-negated-conditions */
  if (value != 1) {} else {} /* expect: avoid-negated-conditions */
  if (value is! String) {} else {} /* expect: avoid-negated-conditions */
  final a = !flag ? 1 : 2; /* expect: avoid-negated-conditions */
  final b = value != null ? 1 : 2; /* expect: avoid-negated-conditions */
  final c = value is! int ? 1 : 2; /* expect: avoid-negated-conditions */
  final list = [if (!flag) 1 else 2]; /* expect: avoid-negated-conditions */
  final set = {if (value != 1) 1 else 2}; /* expect: avoid-negated-conditions */
  final map = {if (value is! int) 1: 2 else 3: 4}; /* expect: avoid-negated-conditions */
  final chained = [if (!flag) 1 else if (value != 1) 2 else 3]; /* expect: avoid-negated-conditions */
}
