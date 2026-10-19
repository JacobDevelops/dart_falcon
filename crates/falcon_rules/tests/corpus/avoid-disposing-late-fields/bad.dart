import 'package:flutter/material.dart' as f;
class Resource {
  void dispose() {}
  void close() {}
  void cancel() {}
}
class S extends f.State<f.StatefulWidget> {
  late Resource unset;
  late final Resource lazy = Resource();
  late Resource stream, subscription;
  late Resource? optional;
  late Resource first = Resource(), second = Resource();
  void dispose() {
    unset.dispose(); /* expect: avoid-disposing-late-fields */
    this.lazy.dispose(); /* expect: avoid-disposing-late-fields */
    stream.close(); /* expect: avoid-disposing-late-fields */
    subscription.cancel(); /* expect: avoid-disposing-late-fields */
    optional?.dispose(); /* expect: avoid-disposing-late-fields */
    optional!.close(); /* expect: avoid-disposing-late-fields */
    unset..dispose(); /* expect: avoid-disposing-late-fields */
    (this.lazy)..close(); /* expect: avoid-disposing-late-fields */
    first.dispose(); /* expect: avoid-disposing-late-fields */
    second.cancel(); /* expect: avoid-disposing-late-fields */
    super.dispose();
  }
}
