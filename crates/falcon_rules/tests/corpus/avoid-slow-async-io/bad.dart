import 'dart:io';

Future<void> fileQueries(File file) async {
  await file.lastModified(); /* expect: avoid-slow-async-io */
  await file.exists(); /* expect: avoid-slow-async-io */
  await file.stat(); /* expect: avoid-slow-async-io */
}

Future<void> directoryQueries(Directory dir) async {
  await dir.exists(); /* expect: avoid-slow-async-io */
  await dir.stat(); /* expect: avoid-slow-async-io */
}

Future<void> linkQueries(Link link) async {
  await link.exists(); /* expect: avoid-slow-async-io */
}

Future<void> staticQueries(String path) async {
  await FileSystemEntity.isDirectory(path); /* expect: avoid-slow-async-io */
  await FileSystemEntity.isFile(path); /* expect: avoid-slow-async-io */
  await FileSystemEntity.isLink(path); /* expect: avoid-slow-async-io */
  await FileSystemEntity.type(path); /* expect: avoid-slow-async-io */
}

Future<void> localFile() async {
  final file = File('data.txt');
  if (await file.exists()) { /* expect: avoid-slow-async-io */
    print('present');
  }
}
