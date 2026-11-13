import 'dart:async'; /* expect: unnecessary-flutter-imports */

class LocalAnnotation {
  const LocalAnnotation();
}

@LocalAnnotation()
@Deprecated('Use another class.')
@pragma('vm:entry-point')
class Example {
  @override
  String toString() => 'Example';
}
