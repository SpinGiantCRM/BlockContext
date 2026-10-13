#!/usr/bin/env bash
# Run as root on the chosen VPS after verifying and unpacking the deployment bundle.
set -euo pipefail
bundle=${1:?Pass the extracted bundle directory}
getent passwd blockcontext >/dev/null || useradd --system --home-dir /var/lib/blockcontext --shell /usr/sbin/nologin blockcontext
getent passwd blockcontext-tunnel >/dev/null || useradd --system --no-create-home --shell /usr/sbin/nologin blockcontext-tunnel
install -d -m 755 /opt/blockcontext /etc/blockcontext
install -d -o blockcontext -g blockcontext -m 700 /var/lib/blockcontext
install -d -m 755 /opt/blockcontext/app /opt/blockcontext/bin /opt/blockcontext/tunnel
cp -a "$bundle/app/." /opt/blockcontext/app/
install -m 755 "$bundle/bin/blockcontext" /opt/blockcontext/bin/blockcontext
cp -a "$bundle/tunnel/." /opt/blockcontext/tunnel/
chown -R root:root /opt/blockcontext
chmod -R go-w /opt/blockcontext
chmod 755 /opt/blockcontext/tunnel/tunnel-client /opt/blockcontext/tunnel/cloudflared
install -o root -g root -m 600 "$bundle/hypixel.env" /etc/blockcontext/hypixel.env
install -o root -g root -m 600 "$bundle/deploy/tunnel.env.example" /etc/blockcontext/tunnel.env.example
install -m 644 "$bundle/deploy/blockcontext.service" /etc/systemd/system/blockcontext.service
install -m 644 "$bundle/deploy/blockcontext-tunnel.service" /etc/systemd/system/blockcontext-tunnel.service
systemd-analyze verify /etc/systemd/system/blockcontext.service /etc/systemd/system/blockcontext-tunnel.service
systemctl daemon-reload
systemctl enable --now blockcontext.service
# Leave the tunnel stopped until its private credentials and permissions are verified.
systemctl is-active blockcontext.service
