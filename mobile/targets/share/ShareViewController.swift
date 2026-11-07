import UIKit

/// The share sheet's "TailCopy" (SPEC 21.4): saves to the inbox and closes.
class ShareViewController: UIViewController {
  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = .clear
    Task { @MainActor in
      _ = await Inbox.save(from: extensionContext)
      extensionContext?.completeRequest(returningItems: nil)
    }
  }
}
