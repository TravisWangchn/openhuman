import requests, json

import os
token = os.environ.get("OPENHUMAN_CORE_TOKEN") or open(r"C:\Users\bigdata\.openhuman-staging\core.token").read().strip()

r = requests.post(
    "http://localhost:7788/rpc",
    json={"jsonrpc": "2.0", "method": "openhuman.config_get", "params": {}, "id": 1},
    headers={"Authorization": f"Bearer {token}"},
    timeout=10,
)
cfg = r.json()["result"]["result"]["config"]

# Key routing fields
for key in ["reasoning_provider","agentic_provider","coding_provider","memory_provider","learning_provider","embeddings_provider","heartbeat_provider","subconscious_provider","default_model","primary_cloud"]:
    print(f"  {key}: {cfg.get(key, 'N/A')}")

print("\n=== Cloud Providers ===")
for cp in cfg.get("cloud_providers", []):
    print(f"  slug={cp['slug']} label={cp['label']} endpoint={cp['endpoint']} auth={cp.get('auth_style','N/A')}")
