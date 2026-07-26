@TestOn('browser')
library;
import 'package:test/test.dart';
void main() {}

class pragma {
  const pragma(String value);
}

@pragma('dart2js:late:trust')
class LocalPragma {}
