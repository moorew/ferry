import Foundation
import UIKit
import UniformTypeIdentifiers

/// Hands a clip from an extension to the app (SPEC 21.2). Extensions never open the
/// database; they write one JSON entry per clip, atomically, with its data files
/// beside it, and the app saves them the next time it runs.
enum Inbox {
  /// The app's App Group, `group.<app id>`. An extension's bundle id is the app's
  /// plus one component (`.share`, `.action`), so this also holds for a build under
  /// someone else's bundle id (docs/BUILD-IOS.md).
  static var group: String? {
    guard let id = Bundle.main.bundleIdentifier, let dot = id.lastIndex(of: ".") else { return nil }
    return "group." + id[..<dot]
  }

  static var directory: URL? {
    guard let group,
      let base = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group)
    else { return nil }
    return base.appendingPathComponent("inbox", isDirectory: true)
  }

  /// Writes one entry. Returns false if there was nothing to save.
  static func save(text: String?, png: Data?, source: String?) throws -> Bool {
    guard text != nil || png != nil, let dir = directory else { return false }
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    let stem = "\(Int(Date().timeIntervalSince1970 * 1000))-\(UUID().uuidString)"
    var formats: [[String: String]] = []
    if let text {
      try Data(text.utf8).write(to: dir.appendingPathComponent("\(stem).txt"), options: .atomic)
      formats.append(["mime": "text/plain", "path": "\(stem).txt"])
    }
    if let png {
      try png.write(to: dir.appendingPathComponent("\(stem).png"), options: .atomic)
      formats.append(["mime": "image/png", "path": "\(stem).png"])
    }
    var entry: [String: Any] = ["v": 1, "formats": formats]
    if let source { entry["source"] = source }
    let json = try JSONSerialization.data(withJSONObject: entry)
    // The JSON goes last: the app only reads entries whose files are all there.
    try json.write(to: dir.appendingPathComponent("\(stem).json"), options: .atomic)
    return true
  }

  /// Reads the first text, URL or image the host app offered.
  static func save(from context: NSExtensionContext?) async -> Bool {
    let providers = (context?.inputItems as? [NSExtensionItem] ?? []).flatMap { $0.attachments ?? [] }
    for p in providers {
      if p.hasItemConformingToTypeIdentifier(UTType.image.identifier),
        let data = try? await p.loadDataRepresentation(for: .image),
        let png = UIImage(data: data)?.pngData()
      {
        return (try? save(text: nil, png: png, source: nil)) ?? false
      }
      if p.hasItemConformingToTypeIdentifier(UTType.url.identifier),
        let url = try? await p.loadItem(forTypeIdentifier: UTType.url.identifier) as? URL
      {
        return (try? save(text: url.absoluteString, png: nil, source: nil)) ?? false
      }
      if p.hasItemConformingToTypeIdentifier(UTType.plainText.identifier),
        let text = try? await p.loadItem(forTypeIdentifier: UTType.plainText.identifier) as? String
      {
        return (try? save(text: text, png: nil, source: nil)) ?? false
      }
    }
    return false
  }
}

private extension NSItemProvider {
  func loadDataRepresentation(for type: UTType) async throws -> Data {
    try await withCheckedThrowingContinuation { cont in
      _ = loadDataRepresentation(forTypeIdentifier: type.identifier) { data, error in
        if let data { cont.resume(returning: data) } else { cont.resume(throwing: error ?? CocoaError(.fileReadUnknown)) }
      }
    }
  }
}
