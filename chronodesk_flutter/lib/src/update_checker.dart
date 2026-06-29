import 'dart:convert';
import 'dart:io';
import 'package:http/http.dart' as http;
import 'package:path/path.dart' as p;

const _repo = 'mrmedani/chronodesk';
const currentVersion = '0.2.5';

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

Future<void> downloadAndApplyUpdate(void Function(int received, int total) onProgress) async {
  final tmpDir = Directory.systemTemp.path;
  final zipFile = File('$tmpDir\\chronodesk_update.zip');

  final uri = Uri.parse('https://github.com/$_repo/releases/latest/download/chronodesk_flutter_windows.zip');
  final client = http.Client();
  try {
    final request = http.Request('GET', uri);
    final response = await client.send(request);
    if (response.statusCode != 200) {
      throw Exception('Download failed: ${response.statusCode}');
    }

    final total = response.contentLength ?? -1;
    final sink = zipFile.openWrite();
    int received = 0;

    await for (final chunk in response.stream) {
      sink.add(chunk);
      received += chunk.length;
      if (total > 0) {
        onProgress(received, total);
      }
    }
    await sink.close();

    _replaceFiles(zipFile.path);
  } finally {
    client.close();
  }
}

void _replaceFiles(String zipPath) {
  final exeDir = File(Platform.resolvedExecutable).parent.path;
  final tmpExtract = '${Directory.systemTemp.path}\\chronodesk_extract';

  final ps = '''
Expand-Archive -Force -LiteralPath "$zipPath" -DestinationPath "$tmpExtract"
''';

  final result = Process.runSync('powershell', ['-Command', ps]);
  if (result.exitCode != 0) {
    throw Exception('Extraction failed: ${result.stderr}');
  }

  final extractDir = Directory(tmpExtract);
  if (!extractDir.existsSync()) {
    throw Exception('Extract directory not found');
  }

  void copyRecursively(Directory dir, String destPath) {
    for (final entry in dir.listSync()) {
      if (entry is File) {
        final dest = File(p.join(destPath, p.basename(entry.path)));
        dest.parent.createSync(recursive: true);
        entry.copySync(dest.path);
      } else if (entry is Directory) {
        copyRecursively(entry, p.join(destPath, p.basename(entry.path)));
      }
    }
  }
  copyRecursively(extractDir, exeDir);

  extractDir.deleteSync(recursive: true);

  final batPath = '$exeDir\\update.bat';
  final bat = '''
@echo off
timeout /t 2 /nobreak >nul
start "" /D "$exeDir" "chronodesk_flutter.exe"
del "%~f0"
''';
  File(batPath).writeAsStringSync(bat);

  Process.start(batPath, [], runInShell: true, mode: ProcessStartMode.detached);
  exit(0);
}
