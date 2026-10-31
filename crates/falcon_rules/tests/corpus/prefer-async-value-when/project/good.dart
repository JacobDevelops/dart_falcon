import 'value.dart';
String render(Model first, Model second) {
  if (first.result.isLoading) return 'loading';
  else if (second.result.hasError) return 'error';
  return 'data';
}
