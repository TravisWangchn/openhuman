import urllib.request, json

correct_token = "de1d7994d60575e4c805d0ff2fad7b40ee9904cd3f0d494b226afd14acbe30be"

def test(token, label):
    body = json.dumps({"jsonrpc":"2.0","method":"core.ping","params":{},"id":1}).encode()
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request("http://127.0.0.1:7788/rpc", data=body, headers=headers)
    try:
        resp = urllib.request.urlopen(req)
        print(f"  [{label}] {resp.read().decode()[:120]}")
    except Exception as e:
        print(f"  [{label}] ERROR: {e}")

test(None, "no token")
test("wrong-token", "wrong token")
test(correct_token, "correct token")
