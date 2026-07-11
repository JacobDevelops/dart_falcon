import 'package:flutter/material.dart';

// Good: use Spacer() instead of Expanded with empty container

class GoodLayout1 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        const Text('Top'),
        const Spacer(),
        const Text('Bottom'),
      ],
    );
  }
}

class GoodLayout2 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        const Text('Left'),
        const Spacer(),
        const Text('Right'),
      ],
    );
  }
}

// Good: Expanded with actual content

class GoodLayout3 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        const Text('A'),
        Expanded(
          child: Container(
            color: Colors.blue,
            child: const Center(
              child: Text('Content'),
            ),
          ),
        ),
        const Text('B'),
      ],
    );
  }
}

class GoodLayout4 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        const Text('Header'),
        Expanded(
          child: ListView(
            children: const [
              ListTile(title: Text('Item 1')),
              ListTile(title: Text('Item 2')),
            ],
          ),
        ),
        const Text('Footer'),
      ],
    );
  }
}

// Good: use Spacer for spacing in column
class GoodLayout5 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        const Text('Start'),
        const Spacer(),
        const Text('Middle'),
        const Spacer(),
        const Text('End'),
      ],
    );
  }
}

// Good: Expanded with actual widget content
class GoodLayout6 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        const Icon(Icons.arrow_left),
        Expanded(
          child: Container(
            padding: const EdgeInsets.all(8),
            child: const Text('Expanded content area'),
          ),
        ),
        const Icon(Icons.arrow_right),
      ],
    );
  }
}

// Good: use Spacer in row for spacing
class GoodLayout7 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        const Text('Left'),
        const Spacer(),
        const Text('Right'),
        const Spacer(),
        const Text('Far Right'),
      ],
    );
  }
}
