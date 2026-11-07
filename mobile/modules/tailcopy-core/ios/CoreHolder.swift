import Foundation
import UIKit
import UniformTypeIdentifiers

/// The one TailCopy core in the app process (SPEC 21.2), and the clipboard around it.
final class CoreHolder: CoreListener, @unchecked Sendable {
  static let shared = CoreHolder()

  private let lock = NSLock()
  private var instance: TailCopy?
  /// Called on the main thread when the store, devices or sync state change.
  var onChanged: (() -> Void)?
  /// `changeCount` after TailCopy's own write, or the last one captured.
  private var seenChangeCount = -1
  private var liveMirror = false

  static var appGroup: String {
    Bundle.main.object(forInfoDictionaryKey: "TailCopyAppGroup") as? String ?? ""
  }

  static var containerURL: URL {
    FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroup)
      ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
  }

  func core() throws -> TailCopy {
    lock.lock()
    defer { lock.unlock() }
    if let core = instance {
      return core
    }
    let base = CoreHolder.containerURL
    let inbox = base.appendingPathComponent("inbox", isDirectory: true)
    try FileManager.default.createDirectory(at: inbox, withIntermediateDirectories: true)
    let core = try TailCopy.open(
      dataDir: base.appendingPathComponent("TailCopy", isDirectory: true).path,
      inboxDir: inbox.path,
      deviceName: UIDevice.current.model,
      listener: self
    )
    instance = core
    Task { await self.loadSettings(core) }
    return core
  }

  private func loadSettings(_ core: TailCopy) async {
    let value = try? await core.setting(key: "live_mirror")
    lock.lock()
    liveMirror = value == "on"
    lock.unlock()
  }

  func setLiveMirror(_ on: Bool) {
    lock.lock()
    liveMirror = on
    lock.unlock()
  }

  // MARK: CoreListener (called on a background thread)

  func changed() {
    DispatchQueue.main.async { self.onChanged?() }
  }

  func liveClip(clipId: String) {
    lock.lock()
    let mirror = liveMirror
    lock.unlock()
    guard mirror else { return }
    Task { @MainActor in
      // Only while the app is in front (SPEC 21.3.6).
      guard UIApplication.shared.applicationState == .active else { return }
      _ = try? await self.copy(clipId: clipId, plainText: false, markUsed: false)
    }
  }

  // MARK: Clipboard

  private static let secretTypes: Set<String> = [
    "org.nspasteboard.ConcealedType",
    "org.nspasteboard.TransientType",
  ]

  /// Reads the pasteboard if it changed since TailCopy last saw it (SPEC 21.4).
  /// Checking `changeCount` and the type list doesn't show the paste prompt;
  /// reading the content does, unless the user allowed pasting from other apps.
  @MainActor
  func captureClipboard() async throws -> String {
    let pb = UIPasteboard.general
    if pb.changeCount == seenChangeCount {
      return "unchanged"
    }
    seenChangeCount = pb.changeCount
    if !CoreHolder.secretTypes.isDisjoint(with: pb.types) {
      return "sensitive"
    }
    var input = ClipInput(text: nil, html: nil, rtf: nil, image: nil)
    if pb.hasStrings {
      input.text = pb.string
      if input.text == nil {
        return "denied"
      }
    }
    if let html = pb.data(forPasteboardType: UTType.html.identifier) {
      input.html = String(data: html, encoding: .utf8)
    }
    input.rtf = pb.data(forPasteboardType: UTType.rtf.identifier)
    if let png = pb.data(forPasteboardType: UTType.png.identifier) {
      input.image = png
    } else if pb.hasImages, let image = pb.image {
      input.image = image.pngData()
    }
    if input.text == nil && input.html == nil && input.rtf == nil && input.image == nil {
      return "empty"
    }
    let saved = try await core().capture(clip: input, sourceApp: nil, sensitive: false, localOnly: false)
    return saved ? "saved" : "empty"
  }

  /// Writes a clip to the pasteboard with every format it has. Clips from other
  /// devices are local-only, so Universal Clipboard doesn't send them on.
  @MainActor
  func copy(clipId: String, plainText: Bool, markUsed: Bool) async throws -> Bool {
    let core = try core()
    let content = try await core.content(clipId: clipId, waitMs: 3000)
    var item: [String: Any] = [:]
    if let text = content.text {
      item[UTType.utf8PlainText.identifier] = text
    }
    if !plainText {
      if let html = content.html {
        item[UTType.html.identifier] = html
      }
      if let rtf = content.rtf {
        item[UTType.rtf.identifier] = rtf
      }
      if let png = content.png {
        item[UTType.png.identifier] = png
      }
    }
    if item.isEmpty {
      return false
    }
    let options: [UIPasteboard.OptionsKey: Any] = content.remote ? [.localOnly: true] : [:]
    UIPasteboard.general.setItems([item], options: options)
    seenChangeCount = UIPasteboard.general.changeCount
    if markUsed {
      try await core.used(clipId: clipId)
    }
    return true
  }
}
