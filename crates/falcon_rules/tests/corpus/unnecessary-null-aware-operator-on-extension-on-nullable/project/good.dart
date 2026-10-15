import 'api.dart' as ext hide NullableValue;

extension NonNullableValue on ext.Value {
  int text() => 0;
}

void hidden(ext.Value? value) {
  value?.text();
}
