# OpenRad server patch for QuotaSpace spaces

The self-hosted coordination server is OpenRad's Go server, built from
`C:\Tools\openrad\server`. QuotaSpace needs two changes in
`internal/server/networks.go`. They are not in upstream OpenRad.

## 1. Keep a private network after its last member leaves

Upstream deletes a private network when the last member leaves. The owner then
cannot switch back to a one-member space, because the network is gone. The
patched leave branch only notifies remaining members and keeps the record. The
explicit delete action is the only way to remove a network.

## 2. Promote a member when the last admin leaves

A private network with other members refuses the last admin's leave. QuotaSpace
switches spaces by leaving the current network, so the owner would be stuck.
The patched branch promotes the oldest remaining member to admin and then
allows the leave.

## Deploy

The VPS runs the binary at `/usr/local/bin/openrad-server` under
`openrad-server.service` with the database at
`/var/lib/openrad-server/openrad-server.db`.

```bash
# copy the patched networks.go to /tmp/openrad-server-src/internal/server/
cd /tmp/openrad-server-src
/usr/local/go/bin/go build -o /tmp/openrad-server-new .
sudo install -m 755 /tmp/openrad-server-new /usr/local/bin/openrad-server
sudo systemctl restart openrad-server
```

The unit must not pass `--public "QuotaVPN"`. The old public baseline is
retired; the app creates a private `qs-...` baseline on the first connect.
Check the unit:

```bash
systemctl cat openrad-server
```

It should end with:

```text
ExecStart=/usr/local/bin/openrad-server --db /var/lib/openrad-server/openrad-server.db serve --public-ip 80.225.89.162
```

## Verify

```bash
sudo /usr/local/bin/openrad-server --db /var/lib/openrad-server/openrad-server.db network list
```

Only the private `qs-...` network should remain. `openrad search QuotaVPN`
must return an empty list.

If the VPS is rebuilt from upstream OpenRad, reapply both edits before starting
the service.
