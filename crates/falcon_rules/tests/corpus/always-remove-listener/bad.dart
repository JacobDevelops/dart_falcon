import 'package:flutter/material.dart';
class A extends State<StatefulWidget> {
 final a = ValueNotifier(0);
 final b = ValueNotifier(0);
 final c = ValueNotifier(0);
 final d = ValueNotifier(0);
 final e = ValueNotifier(0);
 void listener() {}
 void wrong() {}
 void initState() {
   a.addListener(listener); /* expect: always-remove-listener */
   b.addListener(listener); /* expect: always-remove-listener */
   c.addListener(listener); /* expect: always-remove-listener */
   d.addListener(() {}); /* expect: always-remove-listener */
   e.addListener(listener); /* expect: always-remove-listener */
   e..addListener(wrong); /* expect: always-remove-listener */
 }
 void dispose() { b.removeListener(wrong); c.removeListener(wrong); super.dispose(); }
}
class Borrowed extends StatefulWidget {
 final Animation<double> animation;
 final ValueListenable<int> value;
 Borrowed(this.animation, this.value);
 State<Borrowed> createState() => BorrowedState();
}
class BorrowedState extends State<Borrowed> {
 void tick() {}
 void initState() {
   widget.animation.addListener(tick); /* expect: always-remove-listener */
   widget.value.addListener(tick); /* expect: always-remove-listener */
 }
}

class CollectionListeners extends State<StatefulWidget> {
 final Map<String, ValueNotifier<int>> _amounts = {};
 final Map<String, ValueNotifier<int>> _other = {};
 void changed() {}
 void wrong() {}
 void initState() {
   for (final ValueNotifier<int> c in _amounts.values) {
     c.addListener(changed); /* expect: always-remove-listener */
   }
 }
 void dispose() {
   for (final ValueNotifier<int> c in _other.values) {
     c.removeListener(changed);
     c.dispose();
   }
   for (final ValueNotifier<int> c in _amounts.values) {
     c.removeListener(wrong);
   }
   super.dispose();
 }
}

class StoredAliasWrongCleanup extends State<StatefulWidget> {
 ValueNotifier<int>? _authNotifier;
 final other = ValueNotifier<int>(0);
 void changed() {}
 void wrong() {}
 void initState() {
   final notifier = ValueNotifier<int>(0);
   _authNotifier = notifier;
   notifier.addListener(changed); /* expect: always-remove-listener */
 }
 void dispose() {
   _authNotifier?.removeListener(wrong);
   other.dispose();
   super.dispose();
 }
}

class StoredAliasDifferentDisposedField extends State<StatefulWidget> {
 ValueNotifier<int>? _authNotifier;
 final other = ValueNotifier<int>(0);
 void changed() {}
 void initState() {
   final notifier = ValueNotifier<int>(0);
   _authNotifier = notifier;
   notifier.addListener(changed); /* expect: always-remove-listener */
 }
 void dispose() {
   other.dispose();
   super.dispose();
 }
}
