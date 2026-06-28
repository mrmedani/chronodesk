import 'dart:convert';
import 'dart:io';
import 'package:http/http.dart' as http;

const _repo = 'mrmedani/chronodesk';
const currentVersion = '0.2.1';

class UpdateInfo {
  final String version;
  final String url;
  final String notes;

  UpdateInfo({required this.version, required this.url, required this.notes});
}

Future<UpdateInfo?> checkForUpdate() async {
  try {
    final uri = Uri.parse('https://api.github.com/repos/$_repo/releases/latest');
    final resp = await http.get(uri, headers: {'Accept': 'application/vnd.github.v3+json'});
    if (resp.statusCode != 200) return null;

    final data = jsonDecode(resp.body);
    final tag = data['tag_name'] as String? ?? '';
    final clean = tag.startsWith('v') ? tag.substring(1) : tag;

    if (_compareVersions(clean, currentVersion) <= 0) return null;

    return UpdateInfo(
      version: clean,
      url: data['html_url'] ?? '',
      notes: data['body'] ?? '',
    );
  } catch (_) {
    return null;
  }
}

int _compareVersions(String a, String b) {
  final pa = a.split('.').map(int.parse).toList();
  final pb = b.split('.').map(int.parse).toList();
  for (int i = 0; i < 3; i++) {
    final va = i < pa.length ? pa[i] : 0;
    final vb = i < pb.length ? pb[i] : 0;
    if (va != vb) return va.compareTo(vb);
  }
  return 0;
}

Future<String> downloadUpdateZip() async {
  final tmpDir = Directory.systemTemp.path;
  final file = File('$tmpDir\\chronodesk_update.zip');

  final uri = Uri.parse('https://github.com/$_repo/releases/latest/download/chronodesk_flutter_windows.zip');
  final resp = await http.get(uri);
  if (resp.statusCode != 200) throw Exception('Download failed: ${resp.statusCode}');

  await file.writeAsBytes(resp.bodyBytes);
  return file.path;
}

void applyUpdate(String zipPath) {
  final exeDir = File(Platform.resolvedExecutable).parent.path;
  final batPath = '$exeDir\\update.bat';

  final batContent = '''@echo off
timeout /t 3 /nobreak >nul
powershell -Command "Expand-Archive -Force -Path '$zipPath' -DestinationPath '$exeDir'"
start "" "$exeDir\\chronodesk_flutter.exe"
del "%~f0"
''';

  File(batPath).writeAsStringSync(batContent);
  Process.start(batPath, [], runInShell: true, mode: ProcessStartMode.detached);
  exit(0);
}
