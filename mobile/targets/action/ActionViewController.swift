import UIKit

/// "Save to TailCopy" in the share sheet's actions (SPEC 21.4).
class ActionViewController: UIViewController {
  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = .clear
    Task { @MainActor in
      _ = await Inbox.save(from: extensionContext)
      extensionContext?.completeRequest(returningItems: nil)
    }
  }
}
