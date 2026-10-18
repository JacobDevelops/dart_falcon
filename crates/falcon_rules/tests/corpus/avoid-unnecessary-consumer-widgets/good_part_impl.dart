part of 'good_part.dart';

class PartSubclass extends PartBase {
  Widget build(BuildContext context, WidgetRef ref) {
    ref.read;
    return const SizedBox();
  }
}
