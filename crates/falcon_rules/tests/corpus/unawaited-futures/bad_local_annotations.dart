import 'package:meta/meta.dart' show awaitNotRequired;
import 'package:meta/meta.dart' as meta;

Future<void> siblingScopes() async {
  {
    @awaitNotRequired
    Future<void> handleError() async {}
    handleError();
    handleError.call(); /* expect: unawaited-futures */
  }
  {
    Future<void> handleError() async {}
    handleError(); /* expect: unawaited-futures */
    handleError.call(); /* expect: unawaited-futures */
  }
}

Future<void> nestedShadowing() async {
  @awaitNotRequired
  Future<void> handleError() async {}
  {
    Future<void> handleError() async {}
    handleError(); /* expect: unawaited-futures */
    handleError.call(); /* expect: unawaited-futures */
  }
  handleError();
  handleError.call(); /* expect: unawaited-futures */
}

Future<void> prefixedAnnotationCall() async {
  @meta.awaitNotRequired
  Future<void> prefixed() async {}
  prefixed();
  prefixed.call(); /* expect: unawaited-futures */
}
