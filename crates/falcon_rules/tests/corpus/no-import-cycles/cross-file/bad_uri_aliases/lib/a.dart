import 'b.dart'; /* expect: no-import-cycles */
export './b.dart';
import 'package:cycle_fixture/b.dart' as models;
class Repository {}
