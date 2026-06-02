import urllib.request
import os, time

url = "https://github.com/tinyhumansai/openhuman/releases/download/v0.54.0/OpenHuman_0.54.0_x64-setup.exe"
out = r"C:\Users\bigdata\Downloads\OpenHuman_0.54.0_x64-setup.exe"
os.makedirs(os.path.dirname(out), exist_ok=True)

for attempt in range(3):
    try:
        print(f"Attempt {attempt+1}/3...")
        urllib.request.urlretrieve(url, out)
        size = os.path.getsize(out)
        print(f"Downloaded: {size/(1024*1024):.1f} MB")
        break
    except Exception as e:
        print(f"Failed: {e}")
        if attempt < 2:
            time.sleep(5)
