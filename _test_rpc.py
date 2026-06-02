import urllib.request, json

token = "de1d7994d60575e4c805d0ff2fad7b40ee9904cd3f0d494b226afd14acbe30be"
body = json.dumps({"jsonrpc": "2.0", "method": "core.ping", "params": {}, "id": 1}).encode()
req = urllib.request.Request(
    "http://127.0.0.1:7788/rpc",
    data=body,
    headers={
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json",
    },
)
resp = urllib.request.urlopen(req)
print(resp.read().decode())
