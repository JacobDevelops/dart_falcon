import 'package:app//model.dart'; /* expect: avoid-double-slash-imports */
import 'src//one.dart'; /* expect: avoid-double-slash-imports */
import './src///two.dart'; /* expect: avoid-double-slash-imports */
export 'src//three.dart'; /* expect: avoid-double-slash-imports */
import 'base.dart' if (dart.library.io) 'src//io.dart'; /* expect: avoid-double-slash-imports */
import r'src//raw.dart'; /* expect: avoid-double-slash-imports */
import 'src\x2f/escaped.dart'; /* expect: avoid-double-slash-imports */
part 'src//x.dart'; /* expect: avoid-double-slash-imports */
