final int? a = 3; /* expect: unnecessary-nullable-for-final-variable-declarations */
const String? b = 'x'; /* expect: unnecessary-nullable-for-final-variable-declarations */

class C {
  static final int? sf = 5; /* expect: unnecessary-nullable-for-final-variable-declarations */

  void method() {
    final double? d = 1.5; /* expect: unnecessary-nullable-for-final-variable-declarations */
    final List<int>? list = [1, 2]; /* expect: unnecessary-nullable-for-final-variable-declarations */
    final Map<String, int>? m = {'a': 1}; /* expect: unnecessary-nullable-for-final-variable-declarations */
    final bool? flag = true; /* expect: unnecessary-nullable-for-final-variable-declarations */
    print('$d $list $m $flag');
  }
}
