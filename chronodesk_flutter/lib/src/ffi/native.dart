import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';

DynamicLibrary? _lib;
String? _loadError;

void ensureInitialized() {
  if (_lib != null) return;
  try {
    if (Platform.isWindows) {
      _lib = DynamicLibrary.open('chronodesk.dll');
    } else if (Platform.isMacOS) {
      _lib = DynamicLibrary.open('libchronodesk.dylib');
    } else {
      _lib = DynamicLibrary.open('libchronodesk.so');
    }
  } catch (e) {
    _loadError = 'Failed to load native library: $e';
  }
}

DynamicLibrary get _nativeLib {
  ensureInitialized();
  if (_lib == null) throw StateError(_loadError ?? 'Native library not loaded');
  return _lib!;
}

final chronodeskInit = _nativeLib.lookupFunction<
    Void Function(),
    void Function()>('chronodesk_init');

final chronodeskGetPeerId = _lib.lookupFunction<
    Pointer<Utf8> Function(),
    Pointer<Utf8> Function()>('chronodesk_get_peer_id');

final chronodeskFreeString = _lib.lookupFunction<
    Void Function(Pointer<Utf8>),
    void Function(Pointer<Utf8>)>('chronodesk_free_string');

final chronodeskPollEvent = _lib.lookupFunction<
    Pointer<Utf8> Function(),
    Pointer<Utf8> Function()>('chronodesk_poll_event');

final chronodeskConnect = _lib.lookupFunction<
    Void Function(Pointer<Utf8>),
    void Function(Pointer<Utf8>)>('chronodesk_connect');

final chronodeskAccept = _lib.lookupFunction<
    Void Function(),
    void Function()>('chronodesk_accept');

final chronodeskDeny = _lib.lookupFunction<
    Void Function(),
    void Function()>('chronodesk_deny');

final chronodeskDisconnect = _lib.lookupFunction<
    Void Function(),
    void Function()>('chronodesk_disconnect');

final chronodeskGetConfig = _lib.lookupFunction<
    Pointer<Utf8> Function(Pointer<Utf8>),
    Pointer<Utf8> Function(Pointer<Utf8>)>('chronodesk_get_config');

final chronodeskSetConfig = _lib.lookupFunction<
    Void Function(Pointer<Utf8>, Pointer<Utf8>),
    void Function(Pointer<Utf8>, Pointer<Utf8>)>('chronodesk_set_config');

typedef GetFrameNative = Int32 Function(
  Pointer<Pointer<Uint8>>,
  Pointer<Int32>,
  Pointer<Int32>,
  Pointer<Int32>,
);
typedef GetFrameDart = int Function(
  Pointer<Pointer<Uint8>>,
  Pointer<Int32>,
  Pointer<Int32>,
  Pointer<Int32>,
);
final chronodeskGetFrame = _lib.lookupFunction<GetFrameNative, GetFrameDart>('chronodesk_get_frame');

final chronodeskFreeFrame = _lib.lookupFunction<
    Void Function(Pointer<Uint8>),
    void Function(Pointer<Uint8>)>('chronodesk_free_frame');

final chronodeskSendInputMove = _lib.lookupFunction<
    Void Function(Int32, Int32),
    void Function(int, int)>('chronodesk_send_input_move');

final chronodeskGetLog = _lib.lookupFunction<
    Pointer<Utf8> Function(),
    Pointer<Utf8> Function()>('chronodesk_get_log');

final chronodeskSendInputClick = _lib.lookupFunction<
    Void Function(Uint8, Bool),
    void Function(int, bool)>('chronodesk_send_input_click');

String? _readCString(Pointer<Utf8> ptr) {
  if (ptr == nullptr) return null;
  final s = ptr.toDartString();
  chronodeskFreeString(ptr);
  return s;
}

String getPeerId() {
  final ptr = chronodeskGetPeerId();
  return _readCString(ptr) ?? '';
}

String? pollEvent() {
  final ptr = chronodeskPollEvent();
  final s = _readCString(ptr);
  if (s != null && s.isEmpty) return null;
  return s;
}

String getLog() {
  final ptr = chronodeskGetLog();
  return _readCString(ptr) ?? '';
}

bool getFrame(Pointer<Pointer<Uint8>> data, Pointer<Int32> len,
    Pointer<Int32> w, Pointer<Int32> h) {
  return chronodeskGetFrame(data, len, w, h) != 0;
}

String getConfig(String key) {
  final k = key.toNativeUtf8();
  final ptr = chronodeskGetConfig(k);
  malloc.free(k);
  return _readCString(ptr) ?? '';
}

void setConfig(String key, String value) {
  final k = key.toNativeUtf8();
  final v = value.toNativeUtf8();
  chronodeskSetConfig(k, v);
  malloc.free(k);
  malloc.free(v);
}
