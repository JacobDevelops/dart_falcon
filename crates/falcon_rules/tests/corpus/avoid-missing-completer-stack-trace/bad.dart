import 'dart:async';
import 'dart:async' as async;

void errors(Completer<int> completer, Completer<void>? nullable, async.Completer<String> prefixed) {
  completer.completeError('first'); /* expect: avoid-missing-completer-stack-trace */
  completer.completeError('second', null); /* expect: avoid-missing-completer-stack-trace */
  nullable?.completeError('third'); /* expect: avoid-missing-completer-stack-trace */
  prefixed.completeError('fourth'); /* expect: avoid-missing-completer-stack-trace */
  completer..completeError('fifth'); /* expect: avoid-missing-completer-stack-trace */
}
