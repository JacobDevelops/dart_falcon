import 'package:meta/meta.dart' show awaitNotRequired;
import 'package:meta/meta.dart' as meta;

void siblingScopes() {
  {
    @awaitNotRequired
    Future<void> handleError() async {}
    handleError();
    handleError.call(); /* expect: discarded-futures */
  }
  {
    Future<void> handleError() async {}
    handleError(); /* expect: discarded-futures */
    handleError.call(); /* expect: discarded-futures */
  }
}

void nestedShadowing() {
  @awaitNotRequired
  Future<void> handleError() async {}
  {
    Future<void> handleError() async {}
    handleError(); /* expect: discarded-futures */
    handleError.call(); /* expect: discarded-futures */
  }
  handleError();
  handleError.call(); /* expect: discarded-futures */
}

void prefixedAnnotationCall() {
  @meta.awaitNotRequired
  Future<void> prefixed() async {}
  prefixed();
  prefixed.call(); /* expect: discarded-futures */
}
