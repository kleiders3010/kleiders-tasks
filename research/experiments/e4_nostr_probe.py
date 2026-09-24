"""E4: polite acceptance probe of public Nostr relays.

Per relay, with a brand-new throwaway key (no social graph), we send at most
4 small events, all carrying a NIP-40 expiration 1 hour ahead, then a NIP-09
deletion for them:
  1. kind 30078 (NIP-78 app data, addressable), d=<random>, 2 KB opaque base64
  2. the same address again with newer created_at (tests replacement)
  3. kind 31234 (custom addressable), d=<random>, ~60 KB opaque base64 (size)
  4. kind 5 deletion of the above
Then we query the address and report: accepted? replaced (only the newest
returned)? large accepted? Messages returned by the relay are recorded
verbatim (e.g. "blocked: ...", "pow: ...", "rate-limited: ...").
"""
import asyncio
import base64
import hashlib
import json
import os
import sys
import time

import coincurve
import websockets

RELAYS = [
    "wss://relay.damus.io", "wss://nos.lol", "wss://relay.primal.net", "wss://nostr.mom",
    "wss://offchain.pub", "wss://nostr.oxtr.dev", "wss://nostr-pub.wellorder.net",
    "wss://relay.snort.social",
]


def make_event(sk, kind, content, tags, created_at):
    pk = sk.public_key_xonly.format().hex()
    ser = json.dumps([0, pk, created_at, kind, tags, content], separators=(",", ":"), ensure_ascii=False)
    eid = hashlib.sha256(ser.encode()).digest()
    sig = sk.sign_schnorr(eid)
    return {"id": eid.hex(), "pubkey": pk, "created_at": created_at, "kind": kind, "tags": tags,
            "content": content, "sig": sig.hex()}


async def send(ws, ev, timeout=10):
    await ws.send(json.dumps(["EVENT", ev]))
    deadline = time.time() + timeout
    while time.time() < deadline:
        msg = json.loads(await asyncio.wait_for(ws.recv(), timeout=deadline - time.time()))
        if msg[0] == "OK" and msg[1] == ev["id"]:
            return msg[2], msg[3]
        if msg[0] == "NOTICE":
            return False, "NOTICE: " + str(msg[1])
    return None, "timeout"


async def query(ws, flt, timeout=10):
    sub = "p" + os.urandom(4).hex()
    await ws.send(json.dumps(["REQ", sub, flt]))
    out = []
    deadline = time.time() + timeout
    while time.time() < deadline:
        msg = json.loads(await asyncio.wait_for(ws.recv(), timeout=deadline - time.time()))
        if msg[0] == "EVENT" and msg[1] == sub:
            out.append(msg[2])
        elif msg[0] == "EOSE" and msg[1] == sub:
            break
        elif msg[0] == "CLOSED":
            out.append({"closed": msg[2] if len(msg) > 2 else ""})
            break
    await ws.send(json.dumps(["CLOSE", sub]))
    return out


async def probe(url):
    sk = coincurve.PrivateKey()
    pk = sk.public_key_xonly.format().hex()
    now = int(time.time())
    exp = [["expiration", str(now + 3600)]]
    d1 = "kt-probe-" + os.urandom(6).hex()
    d2 = "kt-probe-" + os.urandom(6).hex()
    small = base64.b64encode(os.urandom(1536)).decode()
    large = base64.b64encode(os.urandom(45 * 1024)).decode()  # ~60 KB of base64
    e1 = make_event(sk, 30078, small, [["d", d1]] + exp, now - 5)
    e2 = make_event(sk, 30078, base64.b64encode(os.urandom(1536)).decode(), [["d", d1]] + exp, now)
    e3 = make_event(sk, 31234, large, [["d", d2]] + exp, now)
    res = {"relay": url}
    try:
        async with websockets.connect(url, open_timeout=10, max_size=2**22) as ws:
            res["small_v1"] = await send(ws, e1)
            res["small_v2"] = await send(ws, e2)
            res["large_60kb"] = await send(ws, e3)
            await asyncio.sleep(1)
            got = await query(ws, {"authors": [pk], "kinds": [30078], "#d": [d1]})
            ids = [g.get("id") for g in got if "id" in g]
            res["query_returned"] = len(ids)
            res["replaced_correctly"] = ids == [e2["id"]]
            got3 = await query(ws, {"authors": [pk], "kinds": [31234], "#d": [d2]})
            res["large_readback"] = any(g.get("id") == e3["id"] for g in got3)
            # clean up: NIP-09 deletion by address and id
            dele = make_event(sk, 5, "kt probe cleanup", [["a", f"30078:{pk}:{d1}"], ["a", f"31234:{pk}:{d2}"],
                                                          ["e", e1["id"]], ["e", e2["id"]], ["e", e3["id"]]], now + 1)
            res["deletion"] = await send(ws, dele)
    except Exception as e:
        res["error"] = f"{type(e).__name__}: {e}"
    return res


async def main():
    results = await asyncio.gather(*(probe(u) for u in RELAYS))
    for r in results:
        print(json.dumps(r))


if __name__ == "__main__":
    asyncio.run(main())
