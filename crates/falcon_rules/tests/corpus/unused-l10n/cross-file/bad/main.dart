import 'support.dart';
void main() {
  final l10n = Messages();
  print(l10n.hello);
  print(l10n.greeting('World'));
  final callback = l10n.tearOff;
  print(callback);
  print('${l10n.interpolated}');
  print(Object().l10n.extensionMessage);
  print('stringOnly');
  // commentOnly
}
