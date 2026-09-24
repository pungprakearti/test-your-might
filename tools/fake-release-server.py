"""Stand-in for a GitHub repo's release URLs, for testing self-updates
(src/update.rs, tools/update-e2e.sh). Point the game at it with
TYM_UPDATE_URL=http://127.0.0.1:<port>/<repo>.

Serves <root>/<repo>/ as a repo whose latest release is the tag written in
<root>/<repo>/latest, with that release's files in <root>/<repo>/<tag>/:

    GET /<repo>/releases/latest               -> 302 to /<repo>/releases/tag/<tag>
    GET /<repo>/releases/download/<tag>/<f>   -> <root>/<repo>/<tag>/<f>, or 404

usage: fake-release-server.py <root> <port> [--throttle]
  --throttle  send downloads at ~1 MB/s, so the progress bar is visible
"""

import http.server
import os
import sys
import time

root, port = os.path.abspath(sys.argv[1]), int(sys.argv[2])
throttle = "--throttle" in sys.argv[3:]


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        parts = self.path.strip("/").split("/")
        if len(parts) == 3 and parts[1:] == ["releases", "latest"]:
            latest = os.path.join(root, parts[0], "latest")
            if not os.path.isfile(latest):
                return self.send_error(404)
            tag = open(latest).read().strip()
            self.send_response(302)
            self.send_header("Location", f"http://{self.headers['Host']}/{parts[0]}/releases/tag/{tag}")
            self.send_header("Content-Length", "0")
            return self.end_headers()
        if len(parts) == 5 and parts[1:3] == ["releases", "download"]:
            path = os.path.join(root, parts[0], parts[3], parts[4])
            if not os.path.isfile(path):
                return self.send_error(404)
            data = open(path, "rb").read()
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            for i in range(0, len(data), 64 * 1024):
                self.wfile.write(data[i : i + 64 * 1024])
                if throttle:
                    time.sleep(0.06)
            return
        self.send_error(404)

    def log_message(self, *args):
        pass


http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
