import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as flutter;

Widget listItems(List<String> values) => ListView.builder(
  itemCount: values.length,
  itemBuilder: (context, index) => Text(values[index]),
);

Widget gridItems(List<String> values) => new flutter.GridView.builder(
  itemBuilder: (context, position) {
    return Text((values)[(position)]);
  },
  itemCount: values.length,
);

Widget nullableItems(List<String>? values) => ListView.builder(
  itemCount: values?.length ?? 0,
  itemBuilder: (context, index) => Text(values![index]),
);

Widget unrelatedShadow(List<String> values) {
  void unrelated(List<String> values) { values = []; }
  return ListView.builder(
    itemCount: values.length,
    itemBuilder: (context, index) => Text(values[index]),
  );
}
