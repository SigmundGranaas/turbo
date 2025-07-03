import 'dart:isolate';
import 'dart:typed_data';
import 'package:dio/dio.dart';
import 'package:logging/logging.dart';
import 'package:turbo/features/tile_storage/offline_regions/models/tile_download_job.dart';

/// Data sent from the Orchestrator to the Worker Isolate.
class DownloadTask {
  final SendPort sendPort;
  final TileDownloadJob job;

  DownloadTask(this.sendPort, this.job);
}

/// A class to ferry log records from the worker to the orchestrator.
class WorkerLogRecord {
  final Level level;
  final String message;
  final Object? error;
  final StackTrace? stackTrace;

  WorkerLogRecord(this.level, this.message, [this.error, this.stackTrace]);
}

/// Base class for data sent from the Worker Isolate back to the Orchestrator.
abstract class JobResult {
  final TileDownloadJob job;
  JobResult(this.job);
}

/// Represents a successfully downloaded tile.
class JobSuccess extends JobResult {
  final Uint8List bytes;
  final Duration duration;

  JobSuccess(super.job, this.bytes, this.duration);
}

/// Represents a failed tile download.
class JobFailure extends JobResult {
  final String error;

  JobFailure(super.job, this.error);
}

/// The entry point for the download worker isolate.
/// This function runs in its own memory space and communicates
/// back to the main isolate via the provided SendPort.
void downloadWorkerEntrypoint(DownloadTask task) async {
  final dio = Dio(BaseOptions(
    responseType: ResponseType.bytes,
    connectTimeout: const Duration(seconds: 15),
    receiveTimeout: const Duration(seconds: 15),
  ));

  final stopwatch = Stopwatch()..start();
  final job = task.job;
  final sendPort = task.sendPort;

  sendPort.send(WorkerLogRecord(Level.FINE, 'Starting job for ${job.url}'));

  try {
    final response = await dio.get<Uint8List>(job.url);
    stopwatch.stop();

    if (response.statusCode == 200 && response.data != null) {
      sendPort.send(WorkerLogRecord(Level.FINE, 'Success for ${job.url}'));
      sendPort.send(JobSuccess(job, response.data!, stopwatch.elapsed));
    } else {
      final error = 'Failed with status code: ${response.statusCode}';
      sendPort.send(WorkerLogRecord(Level.WARNING, 'Failure for ${job.url}: $error'));
      sendPort.send(JobFailure(job, error));
    }
  } on DioException catch (e, s) {
    stopwatch.stop();
    final error = e.message ?? 'A network error occurred';
    sendPort.send(WorkerLogRecord(Level.WARNING, 'DioException for ${job.url}: $e', e, s));
    sendPort.send(JobFailure(job, error));
  } catch (e, s) {
    stopwatch.stop();
    final error = e.toString();
    sendPort.send(WorkerLogRecord(Level.SEVERE, 'UNHANDLED CATCH-ALL for ${job.url}', e, s));
    sendPort.send(JobFailure(job, error));
  }
}