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
| `{"op":"query","type":"Line","layer":"Walls"}` | per-entity `{handle,type,layer,…geometry}` (filters + `limit` optional) |
| `{"op":"select","handles":["2B"]}` | set the selection (by `handles`, `type`, or `layer`; `clear` to deselect) → `{"ok":true,"selected":N}` |
| `{"op":"save","path":"out.dwg"}` | `{"ok":true,"saved":"out.dwg"}` (path optional once opened/saved) |

Selection drives modify commands — `select` the targets (e.g. the handles a
`query` returned), then `run("ERASE")`:

```python
ocs.select(type="Line"); ocs.run("ERASE")   # erase every line
ids = [e["handle"] for e in ocs.query(layer="Walls")["entities"]]
ocs.select(handles=ids); ocs.run("ERASE")
```

Every response has `"ok"`; failures carry `"error"`. `run` drives Open CAD
Studio's **own** command system — no separate bindings to maintain — so its
coverage grows with the app.

Interactive draw commands take their points as coordinate tokens; the tool is
started, the points are fed, and it is terminated as if Enter were pressed:

```
{"op":"run","cmd":"LINE 0,0 10,10 10,20"}   → two Line segments
{"op":"run","cmd":"CIRCLE 5,5 3"}           → centre 5,5 radius 3
```

Coordinates are `x,y` or `x,y,z`; `@dx,dy` is relative to the previous point.
Inline-argument commands (`PDMODE 3`, `LAYER Walls`) are passed through as-is.

> Coverage is growing: commands whose options are typed keywords or coordinates
> work; ones that still rely on on-screen picking (object selection by clicking)
> are being wired next.

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
