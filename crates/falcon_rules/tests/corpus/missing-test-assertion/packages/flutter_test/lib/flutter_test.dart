export 'package:test/test.dart';
class WidgetTester { Future<void> pump() async {} }
void testWidgets(String name, Future<void> Function(WidgetTester) callback) {}
