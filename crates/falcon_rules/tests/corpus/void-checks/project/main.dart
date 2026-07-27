import 'helper.dart';

void check(ImportedVoid value, Box<void> box, VoidBox inherited) {
  accept<void>(value); /* expect: void-checks */
  Box<void>.named(1); /* expect: void-checks */
  box.add(1); /* expect: void-checks */
  inherited.add(1); /* expect: void-checks */
  inherited.field = 1; /* expect: void-checks */
}

void checkIndexAssignment(List<void> values) {
  values[0] = 1; /* expect: void-checks */
}
