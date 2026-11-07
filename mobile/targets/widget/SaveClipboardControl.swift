import AppIntents
import SwiftUI
import WidgetKit

/// Control Center, Lock Screen and Action button: "Save clipboard" (SPEC 21.4).
/// Controls can only show SF Symbols or exported custom symbols (D-031).
struct SaveClipboardControl: ControlWidget {
  static let kind = "io.github.moorew.tailcopy.save-clipboard"

  var body: some ControlWidgetConfiguration {
    StaticControlConfiguration(kind: Self.kind) {
      ControlWidgetButton(action: SaveClipboardIntent()) {
        Label("Save clipboard", systemImage: "list.clipboard")
      }
    }
    .displayName("Save clipboard")
    .description("Saves what's on the clipboard to TailCopy.")
  }
}

@main
struct TailCopyWidgets: WidgetBundle {
  var body: some Widget {
    SaveClipboardControl()
  }
}
