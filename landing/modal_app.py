"""Polygone landing page — hosted on Modal (free tier).

Deployment:
    modal deploy modal_app.py

Single static HTML, dark theme, post-quantum network landing.
"""

import subprocess

import modal

APP_NAME = "polygone-landing"

image = (
    modal.Image.debian_slim(python_version="3.12")
    .apt_install("curl")
    .run_commands(
        "curl -fsSL https://deb.nodesource.com/setup_20.x | bash -",
        "apt-get install -y nodejs",
    )
    .workdir("/app")
    .add_local_file("package.json", "/app/package.json", copy=True)
    .run_commands("cd /app && npm install --omit=dev --no-audit --no-fund")
    .add_local_file("server.js", "/app/server.js", copy=True)
    .add_local_file("index.html", "/app/index.html", copy=True)
)

app = modal.App(APP_NAME)


@app.function(
    image=image,
    max_containers=1,
)
@modal.web_server(port=3000, startup_timeout=30)
def serve():
    """Serve static landing page via Express on port 3000."""
    return subprocess.Popen(["node", "server.js"], cwd="/app")
