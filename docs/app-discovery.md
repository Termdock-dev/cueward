# Application discovery and background launch

```sh
cueward app list
cueward app launch --bundle org.example.Editor
cueward app launch --path '/Applications/Example Editor.app'
cueward window list --all-spaces
```

`app list` returns running regular and accessory applications, their PID, bundle identifier, canonical bundle path, active/hidden state, and launch-completion state. It does not enumerate installed applications or provide a durable process identity.

`app launch` accepts exactly one bundle identifier or absolute `.app` path. It validates the executable application bundle before asking macOS to launch it. A matching running instance returns `already_running` without an open or reopen event. Multiple matching instances are rejected; select their windows by PID instead. An application with the same bundle identifier at a different path is rejected rather than substituted.

For a new instance, Cueward requests launch with activation, recent-item additions, and system prompts disabled. It does not hide other applications, send global input, or request a new instance of an already running application. No arguments, environment overrides, document URLs, or executable shell commands are accepted by this interface.

`launched` means the workspace launch callback returned a running application at the requested path. It does not establish that a window exists, loading has finished, or Accessibility inspection will work. Discover its windows and observe them before acting.

The result reports the foreground PID before and after the request. Applications can activate themselves or open visible windows; the launch configuration does not prevent their own behavior. Endpoint observations cannot detect a transient foreground change, and the existing-instance check and launch request are not atomic. Background launch is not a separate desktop session or an input-isolation guarantee.

Timeouts and callback errors can occur after launch was submitted. List running applications before deciding whether to retry. A submitted launch is not automatically cancelled when the CLI exits. The temporary Swift helper requires the macOS command line tools.
