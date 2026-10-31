import 'value.dart';
String render(Model model) {
  if (model.result.isLoading) return 'loading'; /* expect: prefer-async-value-when */
  else if (model.result.hasError) return 'error';
  return 'data';
}
