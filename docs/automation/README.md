# Headless automation API

Open CAD Studio can run without a GUI and be driven over a line-based JSON
protocol — for scripts, batch jobs, or AI agents.

```sh
OpenCADStudio --serve
```

It reads one JSON request per line on **stdin** and writes one JSON response per
line on **stdout**. The active document persists across requests, so a caller
can act → observe → act.

## Protocol

| Request | Response |
|---------|----------|
| `{"op":"new"}` | `{"ok":true,"total":0,"by_type":{}}` |
| `{"op":"open","path":"plan.dwg"}` | entity summary |
| `{"op":"run","cmd":"LAYER Walls"}` | `{"ok":true,"cmd":...,"entities":N,"added":D}` |
| `{"op":"entities"}` | `{"ok":true,"total":N,"by_type":{"Line":42,...}}` |
| `{"op":"save","path":"out.dwg"}` | `{"ok":true,"saved":"out.dwg"}` (path optional once opened/saved) |

Every response has `"ok"`; failures carry `"error"`. `run` drives Open CAD
Studio's **own** command system — no separate bindings to maintain — so its
coverage grows with the app.

> **Status (first increment):** `run` applies synchronous commands (system
> variables, layer ops, …). Pick-based interactive commands (drawing by clicking
> points) need coordinate feeding and are not wired headless yet.

## Python client

[`ocs.py`](ocs.py) is a ~100-line client — nothing to compile:

```python
from ocs import Ocs

with Ocs(binary="OpenCADStudio") as ocs:   # spawns `--serve`
    ocs.open("plan.dwg")
    ocs.run("LAYER Walls")
    print(ocs.entities())
    ocs.save("plan_out.dwg")
```

Any language can speak the same protocol over a subprocess pipe.
