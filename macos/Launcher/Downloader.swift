import Foundation

/// Downloads one byte range of a URL into a file, appending, so a stopped download resumes from
/// what is already on disk. Retries a dropped connection with backoff.
final class RangeDownloader: NSObject, URLSessionDataDelegate {
    private let url: URL
    private let start: Int64
    private let length: Int64
    private let dest: URL
    private let progress: (Int64) -> Void

    private var session: URLSession!
    private var handle: FileHandle?
    private var written: Int64 = 0
    private var continuation: CheckedContinuation<Void, Error>?
    private var task: URLSessionDataTask?

    /// `progress` gets the bytes of the range on disk so far, on an arbitrary queue.
    init(url: URL, start: Int64, length: Int64, to dest: URL, progress: @escaping (Int64) -> Void) {
        self.url = url
        self.start = start
        self.length = length
        self.dest = dest
        self.progress = progress
        super.init()
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 60
        let queue = OperationQueue()
        queue.maxConcurrentOperationCount = 1
        session = URLSession(configuration: config, delegate: self, delegateQueue: queue)
    }

    func run() async throws {
        defer { session.invalidateAndCancel() }
        var attempt = 0
        while true {
            try Task.checkCancellation()
            let have = (try? FileManager.default.attributesOfItem(atPath: dest.path)[.size] as? Int64) ?? 0
            if have >= length { return }
            do {
                try await fetch(from: have)
                return
            } catch is CancellationError {
                throw CancellationError()
            } catch {
                attempt += 1
                if attempt > 8 { throw error }
                // A dropped connection keeps what arrived; back off and resume.
                try await Task.sleep(nanoseconds: UInt64(min(30, 1 << attempt)) * 1_000_000_000)
            }
        }
    }

    private func fetch(from have: Int64) async throws {
        if !FileManager.default.fileExists(atPath: dest.path) {
            FileManager.default.createFile(atPath: dest.path, contents: nil)
        }
        let h = try FileHandle(forWritingTo: dest)
        try h.seekToEnd()
        handle = h
        written = have
        progress(have)

        var req = URLRequest(url: url)
        req.setValue("bytes=\(start + have)-\(start + length - 1)", forHTTPHeaderField: "Range")
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (c: CheckedContinuation<Void, Error>) in
                continuation = c
                let t = session.dataTask(with: req)
                task = t
                t.resume()
            }
        } onCancel: { [weak self] in
            self?.task?.cancel()
        }
    }

    private func finish(_ error: Error?) {
        try? handle?.close()
        handle = nil
        let c = continuation
        continuation = nil
        if let error { c?.resume(throwing: error) } else { c?.resume() }
    }

    // Range is a request header the redirect (archive.org → its storage node) must carry along.
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        var r = request
        r.setValue(task.originalRequest?.value(forHTTPHeaderField: "Range"), forHTTPHeaderField: "Range")
        completionHandler(r)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        // Anything but 206 would append the wrong bytes.
        guard (response as? HTTPURLResponse)?.statusCode == 206 else {
            completionHandler(.cancel)
            finish(RemoteZip.Failure.noRanges(url))
            return
        }
        completionHandler(.allow)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        do {
            try handle?.write(contentsOf: data)
            written += Int64(data.count)
            progress(written)
        } catch {
            dataTask.cancel()
            finish(error)
        }
    }

    // A cancel surfaces here as URLError.cancelled; `run`'s retry sleep then throws the task's
    // CancellationError, so a pause never counts as a failure.
    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        finish(error)
    }
}
