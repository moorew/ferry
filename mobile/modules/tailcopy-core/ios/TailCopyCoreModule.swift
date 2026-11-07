import ExpoModulesCore
import UIKit

private func item(_ c: ClipItem) -> [String: Any?] {
  [
    "id": c.id, "kind": c.kind, "look": c.look, "preview": c.preview, "label": c.label,
    "pinned": c.pinned, "mono": c.mono, "deviceId": c.deviceId, "deviceName": c.deviceName,
    "colorIndex": c.colorIndex, "fromThisDevice": c.fromThisDevice,
    "createdMs": Double(c.createdMs), "byteSize": Double(c.byteSize), "localOnly": c.localOnly,
    "thumbnail": c.thumbnail.map { "file://" + $0 },
  ]
}

private func device(_ d: DeviceItem) -> [String: Any?] {
  [
    "id": d.id, "name": d.name, "role": d.role, "colorIndex": d.colorIndex, "isSelf": d.isSelf,
    "trusted": d.trusted, "connected": d.connected, "caughtUp": d.caughtUp,
    "lastSyncMs": d.lastSyncMs.map(Double.init), "error": d.error, "send": d.send,
    "receive": d.receive,
  ]
}

public class TailCopyCoreModule: Module {
  private func core() throws -> TailCopy {
    try CoreHolder.shared.core()
  }

  public func definition() -> ModuleDefinition {
    Name("TailCopyCore")

    Events("changed")

    OnCreate {
      CoreHolder.shared.onChanged = { [weak self] in self?.sendEvent("changed") }
    }

    AsyncFunction("captureClipboard") { () async throws -> String in
      try await CoreHolder.shared.captureClipboard()
    }

    AsyncFunction("captureText") { (text: String) async throws -> Bool in
      try await self.core().capture(
        clip: ClipInput(text: text, html: nil, rtf: nil, image: nil),
        sourceApp: nil, sensitive: false, localOnly: false)
    }

    AsyncFunction("list") {
      (query: String, offset: Int, limit: Int, today: Double, yesterday: Double) async throws
        -> [[String: Any?]] in
      try await self.core().list(
        query: query, offset: UInt32(offset), limit: UInt32(limit),
        todayStartMs: UInt64(today), yesterdayStartMs: UInt64(yesterday)
      ).map(item)
    }

    AsyncFunction("detail") { (id: String) async throws -> [String: Any?] in
      let c = try await self.core().content(clipId: id, waitMs: 3000)
      return ["text": c.text, "hasImage": c.png != nil, "incomplete": c.incomplete]
    }

    AsyncFunction("copy") { (id: String, plainText: Bool) async throws -> Bool in
      try await CoreHolder.shared.copy(clipId: id, plainText: plainText, markUsed: true)
    }

    AsyncFunction("setPinned") { (id: String, pinned: Bool) async throws in
      try await self.core().setPinned(clipId: id, pinned: pinned)
    }

    AsyncFunction("setLabel") { (id: String, label: String?) async throws in
      try await self.core().setLabel(clipId: id, label: label)
    }

    AsyncFunction("remove") { (id: String) async throws in
      try await self.core().delete(clipId: id)
    }

    AsyncFunction("clearHistory") { (includePinned: Bool) async throws in
      try await self.core().clearHistory(includePinned: includePinned)
    }

    AsyncFunction("devices") { () async throws -> [[String: Any?]] in
      try await self.core().devices().map(device)
    }

    AsyncFunction("setDeviceSync") { (id: String, send: Bool, receive: Bool) async throws in
      try await self.core().setDeviceSync(deviceId: id, send: send, receive: receive)
    }

    AsyncFunction("renameThisDevice") { (name: String) async throws in
      try await self.core().renameThisDevice(name: name)
    }

    AsyncFunction("pair") { (link: String) async throws -> [String: String] in
      let p = try await self.core().pair(link: link)
      return ["deviceId": p.deviceId, "name": p.name]
    }

    AsyncFunction("forget") { (id: String) async throws in
      try await self.core().forget(deviceId: id)
    }

    AsyncFunction("forgetAll") { () async throws in
      try await self.core().forgetAll()
    }

    Function("status") { () throws -> [String: Any] in
      let s = try self.core().status()
      return ["state": s.state, "connected": s.connected]
    }

    Function("tailnetConnected") { () throws -> Bool in
      try self.core().tailnetConnected()
    }

    Function("retry") { () throws in
      try self.core().retry()
    }

    AsyncFunction("setting") { (key: String) async throws -> String? in
      try await self.core().setting(key: key)
    }

    AsyncFunction("setSetting") { (key: String, value: String) async throws in
      try await self.core().setSetting(key: key, value: value)
      if key == "live_mirror" {
        CoreHolder.shared.setLiveMirror(value == "on")
      }
    }

    AsyncFunction("openTailscale") { () async -> Bool in
      // The Tailscale app registers no public URL scheme; open its App Store page,
      // which shows Open when it's installed.
      guard let url = URL(string: "https://apps.apple.com/app/tailscale/id1470499037") else {
        return false
      }
      return await UIApplication.shared.open(url)
    }
  }
}
