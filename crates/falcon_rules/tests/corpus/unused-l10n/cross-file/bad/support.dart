class Messages {
  String get hello => 'Hello';
  String greeting(String name) => name;
  String tearOff(String name) => name;
  String get interpolated => 'Value';
  String get extensionMessage => 'Extended';
}
extension Localization on Object {
  Messages get l10n => Messages();
}
