# Application discovery, launch, and file opening

```sh
cueward app list
cueward app launch --bundle org.example.Editor
cueward app launch --path '/Applications/Example Editor.app'
cueward app open --bundle org.example.Editor --file /tmp/report.txt
cueward app open --path '/Applications/Example Editor.app' --file /tmp/report.txt --new-instance
cueward window list --all-spaces
```

`app list` returns running regular and accessory applications, their PID, bundle identifier, canonical bundle path, active/hidden state, and launch-completion state. It does not enumerate installed applications or provide a durable process identity.

`app launch` accepts exactly one bundle identifier or absolute `.app` path. It validates the executable application bundle before asking macOS to launch it. A matching running instance returns `already_running` without an open or reopen event. Multiple matching instances are rejected; select their windows by PID instead. An application with the same bundle identifier at a different path is rejected rather than substituted.

Bundle lookup requires macOS 12 or later and examines every matching registered application. Canonical paths are deduplicated; multiple executable installations require an explicit `--path`. Older systems can use `--path` directly.

`app launch` requests launch with activation, recent-item additions, and system prompts disabled. It does not hide other applications, send global input, or request a new instance of an already running application. No arguments, environment overrides, document URLs, or executable shell commands are accepted by the launch interface.

`launched` means the workspace launch callback returned a running application at the requested path. It does not establish that a window exists, loading has finished, or Accessibility inspection will work. Discover its windows or use [App AX inspection](app-exploration.md) to explore its exposed roots before acting.

The result reports the foreground PID before and after the request. Applications can activate themselves or open visible windows; the launch configuration does not prevent their own behavior. Endpoint observations cannot detect a transient foreground change, and the existing-instance check and launch request are not atomic. Background launch is not a separate desktop session or an input-isolation guarantee.

Timeouts and callback errors can occur after launch was submitted. List running applications before deciding whether to retry. A submitted launch is not automatically cancelled when the CLI exits. The temporary Swift helper requires the macOS command line tools.

## Open a document in a selected app

`app open` takes one absolute local file path and exactly one app selector (`--bundle` or `--path`). The file must exist and be a readable regular file. Symlinks are resolved; directories, document packages, and remote URLs are not accepted. The app receives the file itself, so work on a disposable copy when the task requires one.

By default, the command opens the file in the unique matching running instance, or starts the selected app. A foreground recipient or multiple matching instances stop the request. Opening in an existing instance holds its shared Cueward input lock through the workspace callback and rechecks the recipient after acquiring the lock. This serializes submission with other input helpers; the app may process the document later.

Use `--new-instance` to explicitly request another process. Cueward checks that the returned PID was not already running before the request. Some apps may refuse or reuse an instance; an error after submission means delivery is uncertain. A new process shares the app's normal settings and storage and may restore existing windows. It is not an isolated profile.

The open request uses the same nonactivating workspace configuration as launch. A `sent_unverified` result means macOS returned a running application at the requested bundle path. It includes that app, the normalized file path, and foreground endpoints. It does not establish that the document loaded, that the app accepted its format, or that edits were saved. Discover the returned app's windows, inspect the document, and verify its contents or resulting file separately.

An app can activate itself or open visible windows even when activation was not requested. Recipient checks and dispatch are not atomic, and the file can change before the app reads it. After any timeout, callback failure, or uncertain effect, observe the app before choosing another action; do not automatically replay an open request. Opening files does not switch Spaces or create a separate desktop session.
