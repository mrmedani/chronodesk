import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:http/http.dart' as http;

const _repo = 'mrmedani/chronodesk';
const currentVersion = '0.3.1';

class UpdateInfo {
  final String version;
  final String notes;

  UpdateInfo({required this.version, required this.notes});
}

Future<UpdateInfo?> checkForUpdate() async {
  final client = http.Client();
  try {
    final uri = Uri.parse('https://api.github.com/repos/$_repo/releases/latest');
    final resp = await client
        .get(uri, headers: {'Accept': 'application/vnd.github.v3+json'})
        .timeout(const Duration(seconds: 10));
    if (resp.statusCode != 200) return null;

    final data = jsonDecode(resp.body);
    final tag = _asString(data['tag_name']);
    if (tag.isEmpty) return null;

    final clean = tag.startsWith('v') || tag.startsWith('V')
        ? tag.substring(1)
        : tag;
    if (clean == currentVersion) return null;
    if (_compareVersions(clean, currentVersion) <= 0) return null;

    return UpdateInfo(
      version: clean,
      notes: _asString(data['body']),
    );
  } finally {
    client.close();
  }
}

String _asString(dynamic v) {
  if (v is String) return v;
  return '';
}

int _compareVersions(String a, String b) {
  final pa = _parseSegments(a);
  final pb = _parseSegments(b);
  for (int i = 0; i < 3; i++) {
    final va = i < pa.length ? pa[i] : 0;
    final vb = i < pb.length ? pb[i] : 0;
    if (va != vb) return va.compareTo(vb);
  }
  return 0;
}

List<int> _parseSegments(String v) {
  final parts = v.split('.');
  final result = <int>[];
  for (final p in parts) {
    final n = int.tryParse(p);
    if (n == null) break;
    result.add(n);
  }
  return result;
}

Future<void> downloadAndApplyUpdate(
    void Function(int received, int total) onProgress) async {
  if (!Platform.isWindows) {
    throw UnsupportedError('Updates are only supported on Windows');
  }

  final tmpDir = Directory.systemTemp.path;
  final installerFile = File('$tmpDir\\chronodesk_setup.exe');

  final uri = Uri.parse(
      'https://github.com/$_repo/releases/latest/download/chronodesk-windows-setup.exe');
  final client = http.Client();
  try {
    final request = http.Request('GET', uri);
    final response =
        await client.send(request).timeout(const Duration(minutes: 5));
    if (response.statusCode != 200) {
      throw Exception('Download failed: ${response.statusCode}');
    }

    final total = response.contentLength ?? -1;
    int received = 0;
    final sink = installerFile.openWrite();

    try {
      await response.stream
          .transform(_progressTransformer((chunkLen) {
            received += chunkLen;
            if (total > 0) onProgress(received, total);
          }))
          .pipe(sink);
    } catch (e) {
      await sink.flush();
      await sink.close();
      if (await installerFile.exists()) {
        await installerFile.delete();
      }
      rethrow;
    }
    await sink.close();
    if (total > 0) onProgress(total, total);

    await _runInstaller(installerFile.path);
  } finally {
    client.close();
  }
}

Future<void> _runInstaller(String installerPath) async {
  final batPath = '${Directory.systemTemp.path}\\chronodesk_update.bat';
  final bat = '''
@echo off
for /l %%i in (1,1,30) do (
  tasklist /FI "IMAGENAME eq chronodesk_flutter.exe" 2>nul | find /I "chronodesk_flutter.exe" >nul
  if errorlevel 1 goto install
  timeout /t 1 /nobreak >nul
)
exit /b 1
:install
start "" /WAIT "$installerPath" /VERYSILENT /CLOSEAPPLICATIONS /NORESTART
del "$installerPath"
del "%~f0"
''';
  await File(batPath).writeAsString(bat);

  final ps =
      'Start-Process -FilePath "$batPath" -Verb RunAs -WindowStyle Hidden';
  await Process.start('powershell', ['-Command', ps],
      runInShell: true, mode: ProcessStartMode.detached);
  exit(0);
}

StreamTransformer<List<int>, List<int>> _progressTransformer(
    void Function(int chunkLen) onData) {
  return StreamTransformer.fromHandlers(
    handleData: (chunk, sink) {
      onData(chunk.length);
      sink.add(chunk);
    },
  );
}
