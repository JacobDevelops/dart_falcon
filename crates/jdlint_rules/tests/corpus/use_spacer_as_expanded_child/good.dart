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
