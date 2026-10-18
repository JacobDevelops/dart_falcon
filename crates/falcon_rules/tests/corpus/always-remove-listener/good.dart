import 'package:flutter/material.dart';
class A extends State<StatefulWidget> {
 final a = ValueNotifier(0);
 final b = ValueNotifier(0);
 final c = ValueNotifier(0);
 final d = ValueNotifier(0);
 final e = ValueNotifier(0);
 void listener() {}
 void initState() {
   a..addListener(listener);
   b.addListener(listener);
   this.c.addListener(this.listener);
   d.addListener(() {});
   e.addListener(listener);
 }
 void dispose() {
   a.removeListener(listener);
   b.dispose();
   c.removeListener(listener);
   d.dispose();
   e.removeListener(listener);
   super.dispose();
 }
}
class Other { final a = ValueNotifier(0); void run() { a.addListener(() {}); } }

class StoredLocalNotifier extends State<StatefulWidget> {
 ValueNotifier<int>? _authNotifier;
 void changed() {}
 void initState() {
   final notifier = ValueNotifier<int>(0);
   _authNotifier = notifier;
   notifier.addListener(changed);
 }
 void dispose() {
   _authNotifier?.dispose();
   super.dispose();
 }
}

class RemovedStoredAlias extends State<StatefulWidget> {
 ValueNotifier<int>? _authNotifier;
 void changed() {}
 void initState() {
   final notifier = ValueNotifier<int>(0);
   _authNotifier = notifier;
   notifier.addListener(changed);
 }
 void dispose() {
   _authNotifier?.removeListener(changed);
   super.dispose();
 }
}

class CollectionListeners extends State<StatefulWidget> {
 final Map<String, ValueNotifier<int>> _amounts = {};
 final Map<String, ValueNotifier<int>> _prices = {};
 void changed() {}
 void initState() {
   for (final c in _amounts.values) {
     c.addListener(changed);
   }
   for (final ValueNotifier<int> c in _amounts.values) {
     c.addListener(changed);
   }
   for (final c in _prices.values) {
     c.addListener(() {});
   }
   for (final ValueNotifier<int> c in _prices.values) {
     c.addListener(() {});
   }
 }
 void dispose() {
   for (final ValueNotifier<int> amount in _amounts.values) {
     amount.removeListener(changed);
   }
   for (final ValueNotifier<int> price in _prices.values) {
     price.dispose();
   }
   super.dispose();
 }
}

class UntraceableListenerOwner extends State<StatefulWidget> {
 void changed() {}
 void register(ValueNotifier<int> notifier) {
   notifier.addListener(changed);
 }
}

class AmbiguousListenerOwner extends State<StatefulWidget> {
 final first = ValueNotifier<int>(0);
 final second = ValueNotifier<int>(0);
 void changed() {}
 void register(bool condition) {
   var notifier = first;
   if (condition) notifier = second;
   notifier.addListener(changed);
 }
}

class AssertedStoredAlias extends State<StatefulWidget> {
 ValueNotifier<int>? _authNotifier;
 ValueNotifier<int>? _otherNotifier;
 void changed() {}
 void initState() {
   final notifier = ValueNotifier<int>(0);
   _authNotifier = notifier;
   notifier.addListener(changed);
   final other = ValueNotifier<int>(0);
   _otherNotifier = other;
   other.addListener(() {});
 }
 void dispose() {
   _authNotifier!.removeListener(changed);
   _otherNotifier!.dispose();
   super.dispose();
 }
}

class UntraceableWrites extends State<StatefulWidget> {
 final first = ValueNotifier<int>(0);
 final Map<String, ValueNotifier<int>> _amounts = {};
 void changed() {}
 void register(bool condition) {
   var indexed = first;
   indexed = _amounts['a']!;
   indexed.addListener(changed);
   var conditional = first;
   conditional = condition ? first : _amounts['b']!;
   conditional.addListener(changed);
   ValueNotifier<int>? compound = first;
   compound ??= _amounts['c']!;
   compound?.addListener(changed);
   var reused = first;
   for (reused in _amounts.values) {
     reused.addListener(changed);
   }
 }
}
