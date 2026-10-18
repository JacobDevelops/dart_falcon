import 'package:flutter/widgets.dart' as flutter;

flutter.Widget mismatchedList(List<String> values, List<String> other) => flutter.ListView.builder(
  itemCount: values.length,
  itemBuilder: (context, index) => flutter.Text(other[index]), /* expect: avoid-unsafe-collection-methods */
);

flutter.Widget mismatchedIndex(List<String> values, int selected) => flutter.GridView.builder(
  itemCount: values.length,
  itemBuilder: (context, index) => flutter.Text(values[selected]), /* expect: avoid-unsafe-collection-methods */
);

flutter.Widget mismatchedCount(List<String> values) => flutter.ListView.builder(
  itemCount: values.length + 1,
  itemBuilder: (context, index) => flutter.Text(values[index]), /* expect: avoid-unsafe-collection-methods */
);

flutter.Widget reassignedList(List<String> values) => flutter.ListView.builder(
  itemCount: values.length,
  itemBuilder: (context, index) {
    values = [];
    return flutter.Text(values[index]); /* expect: avoid-unsafe-collection-methods */
  },
);

flutter.Widget reassignedIndex(List<String> values) => flutter.ListView.builder(
  itemCount: values.length,
  itemBuilder: (context, index) {
    index++;
    return flutter.Text(values[index]); /* expect: avoid-unsafe-collection-methods */
  },
);

flutter.Widget shadowedList(List<String> values) => flutter.ListView.builder(
  itemCount: values.length,
  itemBuilder: (context, index) {
    final List<String> values = [];
    return flutter.Text(values[index]); /* expect: avoid-unsafe-collection-methods */
  },
);

flutter.Widget nestedCallback(List<String> values) => flutter.ListView.builder(
  itemCount: values.length,
  itemBuilder: (context, index) {
    flutter.Widget later() => flutter.Text(values[index]); /* expect: avoid-unsafe-collection-methods */
    return later();
  },
);

flutter.Widget nestedShadowedIndex(List<String> values) => flutter.ListView.builder(
  itemCount: values.length,
  itemBuilder: (context, index) {
    flutter.Widget later(int index) => flutter.Text(values[index]); /* expect: avoid-unsafe-collection-methods */
    return later(100);
  },
);

class ListView {
  ListView.builder({required int itemCount, required flutter.IndexedWidgetBuilder itemBuilder});
}

Object customBuilder(List<String> values) => ListView.builder(
  itemCount: values.length,
  itemBuilder: (context, index) => flutter.Text(values[index]), /* expect: avoid-unsafe-collection-methods */
);

flutter.Widget nullableWrongFallback(List<String>? values) => flutter.ListView.builder(
  itemCount: values?.length ?? 1,
  itemBuilder: (context, index) => flutter.Text(values![index]), /* expect: avoid-unsafe-collection-methods */
);

flutter.Widget blockShadowedIndex(List<String> values) => flutter.ListView.builder(
  itemCount: values.length,
  itemBuilder: (context, index) {
    {
      final index = 100;
      return flutter.Text(values[index]); /* expect: avoid-unsafe-collection-methods */
    }
  },
);

flutter.Widget reassignedAfterConstruction(List<String> values) {
  final widget = flutter.ListView.builder(
    itemCount: values.length,
    itemBuilder: (context, index) => flutter.Text(values[index]), /* expect: avoid-unsafe-collection-methods */
  );
  values = [];
  return widget;
}

flutter.Widget removedDuringBuild(List<String> values) => flutter.ListView.builder(
  itemCount: values.length,
  itemBuilder: (context, index) {
    values.removeLast();
    return flutter.Text(values[index]); /* expect: avoid-unsafe-collection-methods */
  },
);

flutter.Widget clearedAfterConstruction(List<String> values) {
  final widget = flutter.ListView.builder(
    itemCount: values.length,
    itemBuilder: (context, index) => flutter.Text(values[index]), /* expect: avoid-unsafe-collection-methods */
  );
  values.clear();
  return widget;
}

flutter.Widget lengthSetter(List<String> values) => flutter.ListView.builder(
  itemCount: values.length,
  itemBuilder: (context, index) {
    values.length = 0;
    return flutter.Text(values[index]); /* expect: avoid-unsafe-collection-methods */
  },
);

void shrink(List<String> values) { values.clear(); }

flutter.Widget passedToHelper(List<String> values) {
  final widget = flutter.ListView.builder(
    itemCount: values.length,
    itemBuilder: (context, index) => flutter.Text(values[index]), /* expect: avoid-unsafe-collection-methods */
  );
  shrink(values);
  return widget;
}

flutter.Widget aliasedList(List<String> values) {
  final alias = values;
  final widget = flutter.ListView.builder(
    itemCount: values.length,
    itemBuilder: (context, index) => flutter.Text(values[index]), /* expect: avoid-unsafe-collection-methods */
  );
  alias.clear();
  return widget;
}

flutter.Widget cascadedMutation(List<String> values) {
  final widget = flutter.ListView.builder(
    itemCount: values.length,
    itemBuilder: (context, index) => flutter.Text(values[index]), /* expect: avoid-unsafe-collection-methods */
  );
  values..clear();
  return widget;
}
