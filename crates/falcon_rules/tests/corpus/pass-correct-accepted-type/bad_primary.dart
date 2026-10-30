// @dart = 3.13
import 'package:dart_code_metrics_annotations/annotations.dart';

class Zero() {}
class Primary(this.value) {
  @AcceptedTypes({String, bool})
  final Object value;
}
class Named.named(@AcceptedTypes({String}) Object value) {}

void example() {
  Zero();
  Primary(1); /* expect: pass-correct-accepted-type */
  Named.named(false); /* expect: pass-correct-accepted-type */
  Primary('text');
  Primary(true);
  Named.named('text');
}
