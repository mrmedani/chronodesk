import 'package:flutter_test/flutter_test.dart';
import 'package:chronodesk_flutter/src/app.dart';

void main() {
  testWidgets('App smoke test', (WidgetTester tester) async {
    await tester.pumpWidget(const ChronodeskApp());
    expect(find.text('CHRONODESK'), findsOneWidget);
  });
}
