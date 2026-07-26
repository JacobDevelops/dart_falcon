import 'types.dart';

void check(Alpha alpha, Beta beta, IntChild child, Base<String> strings) {
  alpha == beta; /* expect: unrelated-type-equality-checks */
  child == strings; /* expect: unrelated-type-equality-checks */
}
