import 'api.dart' hide calculate;

void hiddenAndPrivate() {
  calculate();
  _privateValue();
}
