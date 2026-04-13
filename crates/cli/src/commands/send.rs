use std::io::Read;
use std::process;

pub(crate) fn dispatch(title: String, body: Option<String>, folder: String, notify: bool) {
    let body = body.unwrap_or_else(|| {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).unwrap_or_default();
        buf
    });

    match cueward_adapter_macos::send::create_note(&title, &body, &folder) {
        Ok(()) => eprintln!("note created in {folder}"),
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }

    if notify {
        let flat = body.replace('\n', " ");
        let preview = if flat.chars().count() > 100 {
            let truncated: String = flat.chars().take(100).collect();
            format!("{truncated}...")
        } else {
            flat
        };
        if let Err(e) = cueward_adapter_macos::send::notify(&title, &preview) {
            eprintln!("warning: notification failed: {e}");
        }
    }
}
