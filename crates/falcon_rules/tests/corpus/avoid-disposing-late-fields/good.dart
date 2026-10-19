import 'package:flutter/material.dart';
class Resource {
  void dispose() {}
  void close() {}
  void cancel() {}
}
class S extends State<StatefulWidget> {
  Resource? optional;
  final eager = Resource();
  late Resource deferred;
  static late Resource shared;
  void cleanup() { deferred.dispose(); }
  void dispose() {
    optional?.dispose();
    eager.dispose();
    shared.dispose();
    final deferred = Resource();
    deferred.dispose();
    void release(Resource deferred) { deferred.dispose(); }
    super.dispose();
  }
}
class Other {
  late Resource resource;
  void dispose() { resource.dispose(); }
}
class Holder extends State<StatefulWidget> {
  late Resource resource;
  void dispose() {
    final other = Other();
    other.resource.dispose();
    super.dispose();
  }
}
