import 'dart:collection' as collection;
import 'helper.dart';

void check(collection.Queue<int> sdkQueue, Queue<int> localQueue, IntItems items) {
  sdkQueue.remove('x'); /* expect: collection-methods-unrelated-type */
  localQueue.remove('x');
  items.contains('x'); /* expect: collection-methods-unrelated-type */
}
