import 'nullable_extension.dart' as ext;

void imported(ext.Value? value) {
  value?.text(); /* expect: unnecessary-null-aware-operator-on-extension-on-nullable */
  ext.NullableValue(value)?.text(); /* expect: unnecessary-null-aware-operator-on-extension-on-nullable */
}
