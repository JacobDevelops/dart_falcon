import 'dart:io' as io;

extension type WrappedDirectory(io.Directory directory) implements io.Directory {}

Future<void> query(WrappedDirectory directory) async {
  await directory.exists(); /* expect: avoid-slow-async-io */
  await io.FileSystemEntity.type('path'); /* expect: avoid-slow-async-io */
}
