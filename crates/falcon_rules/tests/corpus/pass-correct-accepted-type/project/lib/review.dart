// @dart = 3.13
import 'package:dart_code_metrics_annotations/annotations.dart';
import 'package:dart_code_metrics_annotations/api.dart' as external;

extension Accepting on String {
  void send(@AcceptedTypes({String}) Object value) {}
}
class Tag {
  const Tag(@AcceptedTypes({String}) Object value);
}
typedef TagAlias = Tag;
@TagAlias(1) /* expect: pass-correct-accepted-type */
class BadTag {}
@TagAlias('ok')
class GoodTag {}

const label = 1;
enum GoodScoped {
  value(label);
  static const label = 'text';
  const GoodScoped(@AcceptedTypes({String}) Object value);
}
enum BadScoped {
  value(local); /* expect: pass-correct-accepted-type */
  static const local = 1;
  const BadScoped(@AcceptedTypes({String}) Object value);
}
class PrimaryField(@AcceptedTypes({String}) final Object value) {
  PrimaryField.named(this.value);
}
void exercise() {
  'text'.send(1); /* expect: pass-correct-accepted-type */
  'text'..send(1); /* expect: pass-correct-accepted-type */
  Accepting('text').send(1); /* expect: pass-correct-accepted-type */
  PrimaryField.named(1); /* expect: pass-correct-accepted-type */
  external.dependency(1); /* expect: pass-correct-accepted-type */
  external.External(false); /* expect: pass-correct-accepted-type */
  'text'.send('ok');
  'text'..send('ok');
  Accepting('text').send('ok');
  PrimaryField.named('ok');
  external.dependency('ok');
  external.External('ok');
}
