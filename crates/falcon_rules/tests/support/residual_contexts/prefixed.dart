import 'extensions.dart' as ffi show Allocator, Pointer, Item, Allocation, Reference, Bytes;
import 'hidden.dart' as unused hide HiddenBytes, HiddenReference;
import 'hidden.dart' as shown show Marker;
import 'hidden.dart' deferred as lazy hide HiddenBytes, HiddenReference;
void extensions(ffi.Allocator allocator, ffi.Pointer<ffi.Item> pointer,
    ffi.Pointer<int> bytes, bool condition) {
  final ffi.Pointer<ffi.Item> allocated = allocator<ffi.Item>(); /* expect: omit-local-variable-types */
  final ffi.Item item = pointer.ref; /* expect: omit-local-variable-types */
  final List<int> values = bytes.asTypedList(2); /* expect: omit-local-variable-types */
  final (int, int)? pair = condition ? (bytes[0], bytes[1]) : null; /* expect: omit-local-variable-types */
}
