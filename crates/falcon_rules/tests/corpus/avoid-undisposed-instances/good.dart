import 'package:flutter/material.dart';
class Resource {
 void dispose() {}
 static Resource shared() => Resource();
}
class Owner { final resource = Resource(); }
Resource create() => Resource();
Resource createBlock() { return Resource(); }
void consume(Object value) {}
void good() {
 final a = TextEditingController();
 var b = Resource();
 b = Resource();
 Resource()..dispose();
 consume('not disposable');
 a.dispose();
 b.dispose();
}
class InitializedOwner {
 final Resource resource;
 InitializedOwner() : resource = Resource();
}
void wrappers(bool flag, Resource? borrowed) {
 final either = flag ? Resource() : Resource();
 final effective = borrowed ?? Resource();
 final nodes = [FocusNode(), FocusNode()];
 Resource().dispose();
 Resource.shared();
}
void shadowedConstructor() {
 final FocusNode = () => Object();
 consume(FocusNode());
}

void transferredArguments(BuildContext context) {
 Navigator.push(context, MaterialPageRoute<void>(builder: (_) => const SizedBox()));
 consume(TextEditingController());
 consume(ScrollController());
 consume(FocusNode());
 consume(Resource());
 consume(new Resource());
 consume((Resource()));
 consume(Resource()..toString());
 final resource = Resource();
 consume(resource);
 final sink = HiveLogSink();
 LoggerService._(sink: sink);
 LoggerService._(sink: HiveLogSink());
}

class HiveLogSink {
 void dispose() {}
}

class LoggerService {
 final HiveLogSink sink;
 LoggerService._({required this.sink});
}
