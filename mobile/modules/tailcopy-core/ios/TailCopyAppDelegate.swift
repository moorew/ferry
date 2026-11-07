import BackgroundTasks
import ExpoModulesCore
import UIKit

/// Sync follows the app's lifecycle (SPEC 21.3.6): on while the app is active,
/// stopped and the database closed before iOS suspends it, plus an occasional
/// background catch-up when the system allows one.
public class TailCopyAppDelegate: ExpoAppDelegateSubscriber {
  static let refreshTask = "io.github.moorew.tailcopy.refresh"
  private var backgroundTask: UIBackgroundTaskIdentifier = .invalid

  public func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
  ) -> Bool {
    BGTaskScheduler.shared.register(forTaskWithIdentifier: Self.refreshTask, using: nil) { task in
      guard let task = task as? BGAppRefreshTask else { return }
      self.catchUp(task)
    }
    return true
  }

  public func applicationDidBecomeActive(_ application: UIApplication) {
    guard let core = try? CoreHolder.shared.core() else { return }
    core.startSync()
    Task {
      _ = try? await core.ingestInbox()
    }
  }

  public func applicationWillResignActive(_ application: UIApplication) {
    guard let core = try? CoreHolder.shared.core() else { return }
    backgroundTask = application.beginBackgroundTask(withName: "TailCopy suspend") {
      application.endBackgroundTask(self.backgroundTask)
      self.backgroundTask = .invalid
    }
    Task {
      await core.suspend()
      await MainActor.run {
        if self.backgroundTask != .invalid {
          application.endBackgroundTask(self.backgroundTask)
          self.backgroundTask = .invalid
        }
      }
    }
    scheduleRefresh()
  }

  private func scheduleRefresh() {
    let request = BGAppRefreshTaskRequest(identifier: Self.refreshTask)
    request.earliestBeginDate = Date(timeIntervalSinceNow: 30 * 60)
    try? BGTaskScheduler.shared.submit(request)
  }

  /// One catch-up of about 25 seconds, then the database closes again.
  private func catchUp(_ task: BGAppRefreshTask) {
    scheduleRefresh()
    guard let core = try? CoreHolder.shared.core() else {
      task.setTaskCompleted(success: false)
      return
    }
    let work = Task {
      core.startSync()
      do {
        try await Task.sleep(nanoseconds: 25_000_000_000)
      } catch {
        return  // expired: the expiration handler completes the task
      }
      await core.suspend()
      task.setTaskCompleted(success: true)
    }
    task.expirationHandler = {
      work.cancel()
      Task {
        await core.suspend()
        task.setTaskCompleted(success: false)
      }
    }
  }
}
