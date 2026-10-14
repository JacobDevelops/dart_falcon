import 'dart:io';

class Store {
  Future<bool> exists() async => true;
  Future<DateTime> lastModified() async => DateTime.now();
}

void syncQueries(File file, Directory dir, String path) {
  file.lastModifiedSync();
  file.existsSync();
  file.statSync();
  dir.existsSync();
  dir.statSync();
  FileSystemEntity.isDirectorySync(path);
  FileSystemEntity.isFileSync(path);
  FileSystemEntity.isLinkSync(path);
  FileSystemEntity.typeSync(path);
}

Future<void> otherAsync(File file) async {
  await file.readAsString();
  await file.length();
  await file.delete();
}

Future<void> projectType(Store store) async {
  await store.exists();
  await store.lastModified();
}

Future<void> dynamicReceiver(dynamic entity) async {
  await entity.exists();
}

Future<void> statByPath(String path) async {
  await FileStat.stat(path);
}
