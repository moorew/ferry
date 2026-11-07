import AppIntents
import Foundation

// Linked into the app and the widget extension: an intent that opens the app must
// exist in both (SPEC 21.4). Each opens a tailcopy:// route that does the work in
// the app, where reading and writing the pasteboard is allowed.

struct SaveClipboardIntent: AppIntent {
  static let title: LocalizedStringResource = "Save clipboard to TailCopy"
  static let description = IntentDescription("Saves what's on the clipboard to TailCopy.")
  static let isDiscoverable = true
  static let openAppWhenRun = true

  @MainActor
  func perform() async throws -> some IntentResult & OpensIntent {
    .result(opensIntent: OpenURLIntent(URL(string: "tailcopy://capture")!))
  }
}

struct CopyLatestIntent: AppIntent {
  static let title: LocalizedStringResource = "Copy latest from TailCopy"
  static let description = IntentDescription("Puts the newest clip from any of your devices on the clipboard.")
  static let isDiscoverable = true
  static let openAppWhenRun = true

  @MainActor
  func perform() async throws -> some IntentResult & OpensIntent {
    .result(opensIntent: OpenURLIntent(URL(string: "tailcopy://copy-latest")!))
  }
}
