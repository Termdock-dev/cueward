# Safari WebRequest PoC

This PoC checks whether Safari can observe browser-level requests from an existing tab through a temporary web extension. It does not connect to the Cueward CLI or read other sites. Apple documents [temporary extension loading](https://developer.apple.com/documentation/safariservices/running-your-safari-web-extension) and [macOS `webRequest` support](https://developer.apple.com/documentation/safariservices/assessing-your-safari-web-extension-s-browser-compatibility).

The extension requests `webRequest` and `storage` access for `http://127.0.0.1/*` only. It keeps the latest 50 request events in extension storage. Each event contains a URL, method, resource type, timestamp, and status when available. It does not capture request or response bodies.

## Run on Safari 27

1. From the repository root, start the local fixture:

   ```bash
   python3 -m http.server 8765 --bind 127.0.0.1 --directory docs/pocs/safari-webrequest-extension/site
   ```

2. In Safari, enable **Develop > Allow Unsigned Extensions**, then use **Develop > Add Temporary Extension** and select this directory (`docs/pocs/safari-webrequest-extension`). Grant the extension access to `127.0.0.1` if Safari asks.
3. Open `http://127.0.0.1:8765/` in a test tab and click **Fetch sample JSON**.
4. Open the extension's toolbar popup and click **Refresh**. The expected events include a page request and a completed `sample.json` request with status `200`.
5. Remove the temporary extension in Safari Settings after the check. Safari also removes temporary extensions after 24 hours or when Safari quits.

## Validation status

The manifest and JSON fixture parse, and all three JavaScript files pass `node --check`. The fixture returns HTTP `200` for its page, script, and JSON request. Apple's `safari-web-extension-packager` generated a macOS Xcode project, and its Debug app and extension built with Xcode 27 and code signing disabled. The packager warned that the PoC has no icon.

Safari runtime behavior remains unverified until the temporary extension is loaded and its localhost access is granted. This test does not establish that Safari returns response bodies, events from other hosts, or messages to a native CLI. [Production distribution](https://developer.apple.com/documentation/safariservices/distributing-your-safari-web-extension) still needs an app container and signing; Cueward would also need a transport between the extension and CLI.
