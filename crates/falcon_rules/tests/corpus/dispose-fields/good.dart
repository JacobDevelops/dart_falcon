import 'dart:async';
import 'package:flutter/material.dart';
class Resource {
 void dispose() {}
 static Resource sharedInstance() => Resource();
}
class A extends State<StatefulWidget> {
 final a = Resource();
 final b = Resource();
 final controller = TextEditingController();
 final focus = FocusNode();
 final timer = Timer(Duration.zero, () {});
 final cascadedController = TextEditingController()..text = '';
 final singleton = Resource.sharedInstance();
 static final shared = Resource();
 Resource? borrowed;
 void dispose() {
   a.dispose();
   this.b.dispose();
   controller.dispose();
   focus.dispose();
   timer.cancel();
   super.dispose();
 }
}
class Other { final resource = Resource(); }

class Services {
 Future<void> dispose() async {}
 void accept(dynamic value) {}
}

class EagerArgumentCleanup extends State<StatefulWidget> {
 final Services? _services = Services();
 final resource = Services();
 final cascaded = Services();
 final Services? nullableCascaded = Services();
 final eagerCollection = Services();
 final eagerAssignment = Services();
 dynamic pending;
 @override
 void dispose() {
   unawaited(_services?.dispose() ?? Future<void>.value());
   consume(resource.dispose());
   cascaded..dispose();
   nullableCascaded?..dispose();
   consume([eagerCollection.dispose()]);
   pending = eagerAssignment.dispose();
   super.dispose();
 }
}

class CleanupBeforeThrow extends State<StatefulWidget> {
 final resource = Services();
 bool enabled = false;
 @override
 void dispose() {
   consumePair(resource.dispose(), enabled ? throw StateError('stop') : null);
   super.dispose();
 }
}

class CascadeCleanupBeforeThrow extends State<StatefulWidget> {
 final resource = Services();
 bool enabled = false;
 @override
 void dispose() {
   resource..dispose()..accept(enabled ? null : throw StateError('stop'));
   super.dispose();
 }
}

class ParenthesizedReceiverCleanup extends State<StatefulWidget> {
 final resource = Services();
 dynamic receiver;
 @override
 void dispose() {
   (receiver?.resource).accept(resource.dispose());
   super.dispose();
 }
}

void consumePair(dynamic first, dynamic second) {}

void consume(dynamic value) {}
