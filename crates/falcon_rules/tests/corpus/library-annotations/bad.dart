@TestOn('browser') /* expect: library-annotations */
import 'package:test/test.dart';
@pragma('dart2js:late:trust') /* expect: library-annotations */
export 'other.dart';
void main() {}
