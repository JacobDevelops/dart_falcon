// A project-local `SizedBox` is not Flutter's widget.
class SizedBox {
  const SizedBox({this.width, this.height});

  final double? width;
  final double? height;
}

const box = SizedBox(width: 0, height: 0);

SizedBox make() => SizedBox(width: double.infinity, height: double.infinity);
