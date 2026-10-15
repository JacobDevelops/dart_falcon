import 'good_extension_forms.dart' as forms;

void qualifiedExtensions() {
  forms.ImportedOldForms('').old(); /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */
  forms.ImportedGenericForms<int>([1]).old(); /* expect: deprecated-member-use-from-same-package */
}
