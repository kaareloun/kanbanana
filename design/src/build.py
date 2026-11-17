#!/usr/bin/env python3
"""Generates the figcraft script for the linear-board design.

Usage: python3 build.py > script.json
Then:  figcraft cli run script.json --out ../linear-board.figcraft
"""
import json
import re

THEMES = {
    "main": dict(base="#191724", surface="#1f1d2e", overlay="#26233a", muted="#6e6a86",
                 subtle="#908caa", text="#e0def4", love="#eb6f92", gold="#f6c177",
                 rose="#ebbcba", pine="#31748f", foam="#9ccfd8", iris="#c4a7e7",
                 hlLow="#21202e", hlMed="#403d52", hlHigh="#524f67"),
    "moon": dict(base="#232136", surface="#2a273f", overlay="#393552", muted="#6e6a86",
                 subtle="#908caa", text="#e0def4", love="#eb6f92", gold="#f6c177",
                 rose="#ea9a97", pine="#3e8fb0", foam="#9ccfd8", iris="#c4a7e7",
                 hlLow="#2a283e", hlMed="#44415a", hlHigh="#56526e"),
    "dawn": dict(base="#faf4ed", surface="#fffaf3", overlay="#f2e9e1", muted="#9893a5",
                 subtle="#797593", text="#575279", love="#b4637a", gold="#ea9d34",
                 rose="#d7827e", pine="#286983", foam="#56949f", iris="#907aa9",
                 hlLow="#f4ede8", hlMed="#dfdad9", hlHigh="#cecacd"),
}
TOK = set(THEMES["main"])
UI = "Montserrat"
MONO = "JetBrains Mono"
SHADOW = [{"type": "dropShadow", "color": "#00000080", "offset": [0, 12], "radius": 32}]
SHADOW_SM = [{"type": "dropShadow", "color": "#00000059", "offset": [0, 6], "radius": 16}]


def col(c):
    if c is None or c == "none":
        return "none"
    return {"$var": c} if c in TOK else c


def node(type, name=None, props=None, children=(), **kw):
    return {"type": type, "name": name or type, "props": props or {}, "children": list(children), **kw}


def F(children=(), name="Frame", dir=None, gap=0, pad=0, fill=None, w=None, h=None, r=None,
      stroke=None, sw=1, sides=None, x=None, y=None, sh=None, sv=None, main=None, cross=None,
      clip=False, fx=None, wrap=False, extra=None):
    p = {}
    if dir:
        p["layout"] = dir
        p["gap"] = gap
        p["padding"] = pad
        if main:
            p["mainAlign"] = main
        if cross:
            p["crossAlign"] = cross
        if wrap:
            p["wrap"] = True
            p["crossGap"] = gap
    p["fill"] = col(fill)
    if r is not None:
        p["cornerRadius"] = r
    if stroke:
        p["stroke"] = col(stroke)
        p["strokeWeight"] = sw
        p["strokeAlign"] = "inside"
        if sides:
            t, rr, b, l = sides
            p.update(strokeTopWeight=t * sw, strokeRightWeight=rr * sw,
                     strokeBottomWeight=b * sw, strokeLeftWeight=l * sw)
    p["clip"] = clip
    if fx:
        p["effects"] = fx
    if dir:
        p["sizingH"] = sh or ("fixed" if w is not None else "hug")
        p["sizingV"] = sv or ("fixed" if h is not None else "hug")
    else:
        if sh:
            p["sizingH"] = sh
        if sv:
            p["sizingV"] = sv
    if extra:
        p.update(extra)
    kw = {}
    if w is not None:
        kw["width"] = w
    if h is not None:
        kw["height"] = h
    if x is not None:
        kw["x"] = x
    if y is not None:
        kw["y"] = y
    return node("frame", name, p, children, **kw)


def T(s, size=13, wt="Regular", c="text", font=UI, w=None, fill_w=False, lh=None, name=None,
      align=None, extra=None):
    p = {"fontFamily": font, "fontStyle": wt, "fontSize": size, "fill": col(c)}
    if lh:
        p["lineHeight"] = lh
    if align:
        p["textAlign"] = align
    kw = {}
    if fill_w:
        p["sizingH"] = "fill"
        p["textAutoResize"] = "height"
    elif w is not None:
        kw["width"] = w
        p["textAutoResize"] = "height"
    if extra:
        p.update(extra)
    return node("text", name or s[:32], p, text=s, **kw)


def R(w, h, fill=None, r=None, name="Rect", x=None, y=None, stroke=None, sw=1, extra=None):
    p = {"fill": col(fill)}
    if r is not None:
        p["cornerRadius"] = r
    if stroke:
        p.update(stroke=col(stroke), strokeWeight=sw, strokeAlign="inside")
    if extra:
        p.update(extra)
    kw = {"width": w, "height": h}
    if x is not None:
        kw["x"] = x
    if y is not None:
        kw["y"] = y
    return node("rectangle", name, p, **kw)


def E(w, h, fill=None, x=0, y=0, stroke=None, sw=1.5, extra=None, name="Ellipse"):
    p = {"fill": col(fill)}
    if stroke:
        p.update(stroke=col(stroke), strokeWeight=sw, strokeAlign="inside")
    if extra:
        p.update(extra)
    return node("ellipse", name, p, width=w, height=h, x=x, y=y)


def spacer():
    return F(name="Spacer", w=1, h=1, extra={"sizingH": "fill"})


def vspacer():
    return F(name="Spacer", w=1, h=1, extra={"sizingV": "fill"})


# ---------------------------------------------------------------- icons

PATHS = {
    "chevron-down": "M4 6 L8 10 L12 6",
    "chevron-right": "M6 4 L10 8 L6 12",
    "plus": "M8 3 L8 13 M3 8 L13 8",
    "x": "M4 4 L12 12 M12 4 L4 12",
    "refresh": "M13 8 A5 5 0 1 1 11.5 4.5 M12 1.5 L12 4.8 L8.7 4.8",
    "external": "M9 3 L13 3 L13 7 M13 3 L7.5 8.5 M11 10 L11 13 L3 13 L3 5 L6 5",
    "image": "M2.5 3.5 L13.5 3.5 L13.5 12.5 L2.5 12.5 Z M2.5 11 L6 7.5 L9 10.5 L10.5 9 L13.5 12",
    "pencil": "M3 13 L3.5 10.5 L10.5 3.5 L12.5 5.5 L5.5 12.5 Z",
    "comment": "M2.5 3.5 L13.5 3.5 L13.5 11 L7 11 L4 13.5 L4 11 L2.5 11 Z",
    "check": "M3.5 8.5 L6.5 11.5 L12.5 4.5",
    "copy": "M5.5 5.5 L13 5.5 L13 13 L5.5 13 Z M3 10.5 L3 3 L10.5 3",
    "reply": "M6 4 L2.5 7.5 L6 11 M2.5 7.5 L9.5 7.5 A3.5 3.5 0 0 1 13 11 L13 12.5",
    "sliders": "M2.5 5 L13.5 5 M2.5 11 L13.5 11 M10 3 L10 7 M6 9 L6 13",
    "key": "M7 9 A3 3 0 1 1 7 8.99 M9.2 6.8 L13.5 2.5 M11.5 4.5 L13 6",
    "board": "M2.5 3 L5.5 3 L5.5 13 L2.5 13 Z M6.5 3 L9.5 3 L9.5 10 L6.5 10 Z M10.5 3 L13.5 3 L13.5 8 L10.5 8 Z",
    "grip": "M6 4 L6 4.01 M10 4 L10 4.01 M6 8 L6 8.01 M10 8 L10 8.01 M6 12 L6 12.01 M10 12 L10 12.01",
    "branch": "M3.5 3.5 A1.5 1.5 0 1 0 6.5 3.5 A1.5 1.5 0 1 0 3.5 3.5 "
              "M3.5 12.5 A1.5 1.5 0 1 0 6.5 12.5 A1.5 1.5 0 1 0 3.5 12.5 "
              "M9.5 4.5 A1.5 1.5 0 1 0 12.5 4.5 A1.5 1.5 0 1 0 9.5 4.5 "
              "M5 5 L5 11 M11 6 A4.5 4.5 0 0 1 6.2 10.6",
    "mr": "M3 3.5 A1.5 1.5 0 1 0 6 3.5 A1.5 1.5 0 1 0 3 3.5 "
          "M3 12.5 A1.5 1.5 0 1 0 6 12.5 A1.5 1.5 0 1 0 3 12.5 "
          "M10 12.5 A1.5 1.5 0 1 0 13 12.5 A1.5 1.5 0 1 0 10 12.5 "
          "M4.5 5 L4.5 11 M11.5 11 L11.5 6.5 A2 2 0 0 0 9.5 4.5 L7 4.5 M8.5 3 L7 4.5 L8.5 6",
    "folder": "M2.5 4 L6.5 4 L8 5.5 L13.5 5.5 L13.5 12.5 L2.5 12.5 Z",
    "asterisk": "M8 2.5 L8 13.5 M3.2 5.25 L12.8 10.75 M12.8 5.25 L3.2 10.75",
    "terminal": "M3 4.5 L6 7.5 L3 10.5 M8 11 L13 11",
    "arrow": "M3 8 L13 8 M9 4 L13 8 L9 12",
    "link": "M7 9 L9 7 M6.5 6 L5 7.5 A2.5 2.5 0 0 0 8.5 11 L10 9.5 "
            "M9.5 10 L11 8.5 A2.5 2.5 0 0 0 7.5 5 L6 6.5",
    "server": "M2.5 3 L13.5 3 L13.5 7.5 L2.5 7.5 Z M2.5 8.5 L13.5 8.5 L13.5 13 L2.5 13 Z",
    "lock": "M4 7.5 L12 7.5 L12 13.5 L4 13.5 Z M5.5 7.5 L5.5 5 A2.5 2.5 0 0 1 10.5 5 L10.5 7.5",
}


def icon(name, c="subtle", s=16, sw=1.5):
    fill = "none"
    p = {"stroke": col(c), "strokeWeight": sw, "fill": fill, "strokeCap": "round",
         "strokeJoin": "round"}
    vec = node("vector", "path", p, path=scale_path(PATHS[name], s / 16), x=0, y=0)
    return F([vec], name=f"icon/{name}", w=s, h=s)


def scale_path(d, k):
    """Scales absolute path data. Arc rotation and flags stay as they are."""
    if k == 1:
        return d
    out, cmd, i = [], None, 0
    for tok in re.findall(r"[A-Za-z]|-?\d*\.?\d+", d):
        if tok.isalpha():
            cmd, i = tok, 0
            out.append(tok)
            continue
        keep = cmd == "A" and i % 7 in (2, 3, 4)
        out.append(tok if keep else f"{float(tok) * k:.2f}")
        i += 1
    return " ".join(out)


def status_icon(kind, s=14):
    if kind == "backlog":
        ch = [E(s, s, None, stroke="muted", extra={"dashes": [2, 2]})]
    elif kind == "todo":
        ch = [E(s, s, None, stroke="subtle")]
    elif kind == "progress":
        ch = [E(s, s, None, stroke="gold"),
              E(s / 2, s / 2, "gold", x=s / 4, y=s / 4, extra={"arcStart": -90, "arcSweep": 180})]
    elif kind == "review":
        ch = [E(s, s, None, stroke="foam"),
              E(s / 2, s / 2, "foam", x=s / 4, y=s / 4, extra={"arcStart": -90, "arcSweep": 270})]
    elif kind == "done":
        ch = [E(s, s, "iris"),
              node("vector", "check", {"stroke": col("base"), "strokeWeight": 1.6, "fill": "none",
                                       "strokeCap": "round", "strokeJoin": "round"},
                   path="M4 7.2 L6.2 9.4 L10 5", x=0, y=0)]
    else:  # canceled
        ch = [E(s, s, "muted"),
              node("vector", "x", {"stroke": col("base"), "strokeWeight": 1.6, "fill": "none",
                                   "strokeCap": "round"},
                   path="M4.8 4.8 L9.2 9.2 M9.2 4.8 L4.8 9.2", x=0, y=0)]
    return F(ch, name=f"status/{kind}", w=s, h=s)


def priority_icon(p, s=14):
    if p == "urgent":
        return F([T("!", 10, "Bold", "base")], name="priority/urgent", dir="horizontal", w=s, h=s,
                 r=3, fill="love", main="center", cross="center")
    if p == "none":
        return F([R(3, 1.5, "muted", r=1, x=1, y=6.25), R(3, 1.5, "muted", r=1, x=5.5, y=6.25),
                  R(3, 1.5, "muted", r=1, x=10, y=6.25)], name="priority/none", w=s, h=s)
    n = {"high": 3, "medium": 2, "low": 1}[p]
    bars = []
    for i, bh in enumerate([5, 8, 11]):
        bars.append(R(3, bh, "subtle" if i < n else "hlHigh", r=1, x=1 + i * 4.5, y=12.5 - bh))
    return F(bars, name=f"priority/{p}", w=s, h=s)


PRIORITY_NAME = {"urgent": "Urgent", "high": "High", "medium": "Medium", "low": "Low",
                 "none": "No priority"}

USERS = {
    "KA": ("Kaarel", "iris", "base"),
    "MT": ("Mari Tamm", "foam", "base"),
    "JK": ("Joonas Kask", "gold", "base"),
    "LS": ("Liis Saar", "rose", "base"),
    "AP": ("Andres Pärn", "pine", "text"),
}


def avatar(u, s=20):
    if u is None:
        return F([E(s, s, None, stroke="muted", sw=1.2, extra={"dashes": [2, 2]})],
                 name="avatar/none", w=s, h=s)
    _, c, tc = USERS[u]
    return F([T(u, round(s * 0.4), "SemiBold", tc)], name=f"avatar/{u}", dir="horizontal",
             w=s, h=s, r=s / 2, fill=c, main="center", cross="center")


def kbd(s):
    return F([T(s, 10, "Medium", "subtle", MONO)], name="kbd", dir="horizontal", pad=[1, 5, 1, 5],
             r=4, fill="overlay", stroke="hlMed")


LABELS = {"Bug": "love", "Feature": "iris", "Improvement": "foam", "Design": "rose",
          "Backend": "pine", "Frontend": "gold"}


def label(name, size=11):
    return F([E(8, 8, LABELS[name]), T(name, size, "Medium", "subtle")], name=f"label/{name}",
             dir="horizontal", gap=5, cross="center")


def btn(text, kind="secondary", ic=None, k=None):
    tc = {"primary": "base", "secondary": "text", "ghost": "subtle"}[kind]
    fill = {"primary": "iris", "secondary": "overlay", "ghost": None}[kind]
    ch = []
    if ic:
        ch.append(icon(ic, tc, 14))
    ch.append(T(text, 12, "SemiBold", tc))
    if k:
        ch.append(T(k, 10, "Medium", "base" if kind == "primary" else "muted", MONO,
                    extra={"opacity": 0.7}))
    return F(ch, name=f"button/{text}", dir="horizontal", gap=6, pad=[6, 12, 6, 12], r=6,
             fill=fill, stroke="hlMed" if kind == "secondary" else None, cross="center")


def icon_btn(ic, c="subtle"):
    return F([icon(ic, c)], name=f"iconbutton/{ic}", dir="horizontal", pad=6, r=6)


def checkbox(on, s=14):
    if on:
        return F([node("vector", "check", {"stroke": col("base"), "strokeWeight": 1.6,
                                           "fill": "none", "strokeCap": "round",
                                           "strokeJoin": "round"},
                       path="M3.5 7.2 L6 9.6 L10.5 4.6", x=0, y=0)],
                 name="checkbox/on", w=s, h=s, r=3, fill="iris")
    return F(name="checkbox/off", w=s, h=s, r=3, stroke="hlHigh", sw=1.5)


def segmented(options, active):
    items = []
    for o in options:
        on = o == active
        items.append(F([T(o, 12, "SemiBold" if on else "Medium", "text" if on else "subtle")],
                       name=f"segment/{o}", dir="horizontal", pad=[4, 10, 4, 10], r=4,
                       fill="overlay" if on else None))
    return F(items, name="segmented", dir="horizontal", pad=2, gap=2, r=6, fill="surface",
             stroke="hlLow")


# ---------------------------------------------------------------- data

STATES = [("Backlog", "backlog"), ("Todo", "todo"), ("In Progress", "progress"),
          ("In Review", "review"), ("Done", "done")]


def I(id, title, prio, user, labels=(), comments=0, images=False, project=None):
    return dict(id=id, title=title, prio=prio, user=user, labels=list(labels),
                comments=comments, images=images, project=project)


WEB = {
    "Backlog": [I("WEB-231", "Season switch banner for winter tyres", "low", None, ["Design"]),
                I("WEB-228", "Merchant Center feed misses GTIN for studded tyres", "medium", "MT",
                  ["Bug"], 1)],
    "Todo": [I("WEB-226", "Parcel price rounds to 2.99 instead of 3.00", "high", "KA",
               ["Bug", "Backend"], 2),
             I("WEB-219", "Add tyre hotel storage step to the booking flow", "medium", "LS",
               ["Feature"]),
             I("WEB-217", "Search Console: duplicate canonical on /tyres pages", "low", "AP",
               ["Improvement"])],
    "In Progress": [I("WEB-214", "Refund webhook marks the order paid twice", "urgent",
                      "KA", ["Bug", "Backend"], 3, True),
                    I("WEB-210", "Tyre size filter drops 17 inch rims", "high", "JK",
                      ["Bug", "Frontend"], 1)],
    "In Review": [I("WEB-208", "Booking SMS goes out in English for the et locale", "medium",
                    "KA", ["Bug"], 2)],
    "Done": [I("WEB-205", "Export bookings to CSV from the admin", "medium", "MT", ["Feature"]),
             I("WEB-201", "Show stock per warehouse on the product page", "low", "JK",
               ["Feature", "Frontend"], 4)],
}
PLT = {
    "Backlog": [I("PLT-88", "Move image uploads to Hetzner object storage", "none", None,
                  ["Backend"])],
    "Todo": [I("PLT-91", "Nightly Postgres backup check fails silently", "high", "AP", ["Bug"], 1)],
    "In Progress": [I("PLT-93", "Upgrade TanStack Start to 1.130", "medium", "KA",
                      ["Improvement"], 2)],
    "In Review": [I("PLT-90", "Rate limit the login endpoint", "high", "JK", ["Backend"]),
                  I("PLT-86", "Sentry: drop noisy hydration warnings", "low", "LS",
                    ["Improvement"])],
    "Done": [I("PLT-84", "Caddy config for staging subdomains", "none", "MT")],
}

# Grouped by project, assigned to me.
P_WEB = {
    "Backlog": [],
    "Todo": [WEB["Todo"][0]],
    "In Progress": ["PLACEHOLDER",
                    I("WEB-233", "Show the delivery estimate on the cart page", "medium", "KA",
                      ["Feature", "Frontend"])],
    "In Review": ["DROP", WEB["In Review"][0]],
    "Done": [I("WEB-199", "Fix VAT on credit notes", "medium", "KA", ["Bug"], 3)],
}
P_INFRA = {
    "Backlog": [],
    "Todo": [I("PLT-95", "Rotate Linear API keys in CI", "low", "KA", ["Improvement"])],
    "In Progress": [PLT["In Progress"][0]],
    "In Review": [],
    "Done": [I("PLT-87", "Add an uptime check for the booking API", "low", "KA", ["Feature"], 1)],
}
P_NONE = {
    "Backlog": [I("WEB-190", "Clean up old tyre size redirects", "low", "KA", ["Improvement"])],
    "Todo": [], "In Progress": [], "In Review": [], "Done": [],
}

# Dev state per issue, as Claude reports it over MCP.
DEV = {
    "WEB-226": dict(stage="Local", ref="parcel-rounding"),
    "WEB-214": dict(stage="Local", ref="refund-webhook"),
    "WEB-210": dict(stage="MR", ref="!412"),
    "WEB-208": dict(stage="Prod"),
    "WEB-205": dict(stage="Prod"),
    "WEB-201": dict(stage="Merged"),
    "WEB-199": dict(stage="Prod"),
    "PLT-93": dict(stage="Local", ref="upgrade-tanstack"),
    "PLT-90": dict(stage="MR", ref="!88"),
}
for _g in (WEB, PLT, P_WEB, P_INFRA, P_NONE):
    for _items in _g.values():
        for _it in _items:
            if isinstance(_it, dict) and _it["id"] in DEV:
                _it["dev"] = DEV[_it["id"]]

STAGE_COLOR = {"Local": "gold", "MR": "rose", "Merged": "iris", "Staging": "iris",
               "Prod": "foam"}

COLW = 256
COLGAP = 12
DRAG_H = 134
BOARD_X = 64
BOARD_PAD = 24


# ---------------------------------------------------------------- board parts

def card(it, selected=False, w=None, extra=None, fx=None):
    row1 = F([T(it["id"], 11, "Medium", "muted", MONO), spacer(), avatar(it["user"])],
             name="row", dir="horizontal", cross="center", sh="fill")
    meta_items = [priority_icon(it["prio"])] + [label(l) for l in it["labels"]]
    tail = []
    if it["images"]:
        tail.append(icon("image", "muted", 14))
    if it["comments"]:
        tail.append(F([icon("comment", "muted", 14), T(str(it["comments"]), 11, "Medium", "muted")],
                      name="comments", dir="horizontal", gap=3, cross="center"))
    meta = F(meta_items + [spacer()] + tail, name="meta", dir="horizontal", gap=10,
             cross="center", sh="fill")
    title = T(it["title"], 13, "Medium", "text", fill_w=True, lh=18)
    rows = [row1, title, meta]
    if it.get("dev"):
        rows.append(dev_row(it["dev"]))
    return F(rows, name=f"card/{it['id']}", dir="vertical", gap=8, pad=12,
             fill="overlay" if selected else "surface", r=8,
             stroke="iris" if selected else "hlLow", sw=1, w=w,
             sh=None if w else "fill", fx=fx, extra=extra)


def trunc(s, size, wt, c, font):
    """One-line text that fills its row and ends in an ellipsis."""
    p = {"fontFamily": font, "fontStyle": wt, "fontSize": size, "fill": col(c),
         "sizingH": "fill", "textAutoResize": "truncate"}
    return node("text", s[:32], p, text=s, height=round(size * 1.3))


def dev_row(d):
    c = STAGE_COLOR[d["stage"]]
    items = [E(6, 6, c), T(d["stage"], 11, "SemiBold", c)]
    if d.get("ref"):
        items.append(trunc(d["ref"], 11, "Medium", "muted", MONO))
    return F(items, name="dev", dir="horizontal", gap=6, cross="center", sh="fill")


def placeholder():
    return F([], name="drag/placeholder", w=COLW, h=DRAG_H, r=8, stroke="hlHigh", sw=1,
             extra={"dashes": [4, 4], "sizingH": "fill"})


def drop_slot():
    return F([], name="drag/drop-slot", w=COLW, h=DRAG_H, r=8, fill="hlLow", stroke="iris", sw=1,
             extra={"dashes": [4, 4], "sizingH": "fill"})


def column(items, highlight=False):
    ch = []
    for it in items:
        if it == "PLACEHOLDER":
            ch.append(placeholder())
        elif it == "DROP":
            ch.append(drop_slot())
        else:
            ch.append(card(it, selected=it.get("_selected", False)))
    return F(ch, name="column", dir="vertical", gap=8, w=COLW, pad=[0, 0, 4, 0] if not highlight
             else [6, 6, 6, 6], fill="hlLow" if highlight else None, r=8 if highlight else None)


def lane(title, key, data, collapsed=False, drop_col=None):
    count = sum(1 for s in data.values() for i in s if isinstance(i, dict) or i == "PLACEHOLDER")
    head = [icon("chevron-right" if collapsed else "chevron-down", "muted", 14),
            T(title, 13, "SemiBold", "text")]
    if key:
        head.append(T(key, 11, "Medium", "muted", MONO))
    head.append(T(f"{count} issue" + ("" if count == 1 else "s"), 12, "Medium", "muted"))
    header = F(head, name="lane/header", dir="horizontal", gap=8, cross="center", h=28)
    ch = [header]
    if not collapsed:
        cols = [column(data[s], highlight=(s == drop_col)) for s, _ in STATES]
        ch.append(F(cols, name="lane/columns", dir="horizontal", gap=COLGAP, cross="min"))
    return F(ch, name=f"lane/{title}", dir="vertical", gap=8)


def column_headers(groups):
    hs = []
    for s, kind in STATES:
        n = sum(1 for g in groups for i in g.get(s, []) if isinstance(i, dict))
        hs.append(F([status_icon(kind), T(s, 12, "SemiBold", "text"),
                     T(str(n), 12, "Medium", "muted"), spacer(),
                     icon("plus", "muted", 14) if False else spacer()],
                    name=f"colheader/{s}", dir="horizontal", gap=8, cross="center", w=COLW, h=36))
    return F(hs, name="column-headers", dir="horizontal", gap=COLGAP, w=1376,
             pad=[0, BOARD_PAD, 0, BOARD_PAD], cross="center", h=44, fill="base",
             stroke="hlLow", sides=(0, 0, 1, 0))


def board(lanes, groups, h=844):
    body = F(lanes, name="lanes", dir="vertical", gap=20, pad=[16, BOARD_PAD, 24, BOARD_PAD],
             w=1376)
    return F([column_headers(groups), body], name="board", dir="vertical", w=1376, h=h,
             fill="base", clip=True, x=BOARD_X, y=56)


WORKSPACES = [("A", "acme", "iris", "Alt 1"), ("N", "northwind", "foam", "Alt 2"),
              ("B", "bluebird", "gold", "Alt 3")]


def ws_tile(letter, accent, active=False, s=40):
    return F([T(letter, 16, "SemiBold", accent)], name=f"workspace/{letter}", dir="horizontal",
             w=s, h=s, r=10, fill="surface" if active is False else "overlay",
             stroke="iris" if active else None, sw=1.5, main="center", cross="center")


def rail(active="acme", count=3, adding=False):
    items = []
    for letter, name, accent, key in WORKSPACES[:count]:
        items.append(F([ws_tile(letter, accent, active == name),
                        T(key, 9, "Medium", "subtle" if active == name else "muted", MONO)],
                       name=f"rail/{name}", dir="vertical", gap=5, cross="center"))
    add = F([icon("plus", "iris" if adding else "subtle")], name="workspace/add",
            dir="horizontal", w=40, h=40, r=10, fill="overlay" if adding else None,
            stroke="iris" if adding else "hlHigh", sw=1.5 if adding else 1, main="center",
            cross="center", extra=None if adding else {"dashes": [3, 3]})
    items += [vspacer(), add]
    return F(items, name="rail", dir="vertical", gap=14, pad=[16, 0, 16, 0], w=64, h=900,
             fill="base", stroke="hlLow", sides=(0, 1, 0, 0), cross="center", x=0, y=0)


def topbar(group="Team", mine=False, ws="acme", teams="3 teams", synced="Synced 12 s ago",
           ):
    return F([
        F([T(ws, 15, "SemiBold", "text"), T(teams, 12, "Medium", "muted")], name="workspace",
          dir="horizontal", gap=10, cross="center"),
        R(1, 20, "hlMed", name="divider"),
        F([T("Group by", 12, "Medium", "muted"), segmented(["Team", "Project"], group)],
          name="group-by", dir="horizontal", gap=8, cross="center"),
        F([checkbox(mine), T("Assigned to me", 12, "Medium", "text" if mine else "subtle"),
           kbd("M")], name="filter/mine", dir="horizontal", gap=8, cross="center",
          pad=[5, 10, 5, 8], r=6, fill="overlay" if mine else None,
          stroke="hlMed" if mine else None),
        spacer(),
        F([E(6, 6, "foam"), T(synced, 12, "Medium", "muted")], name="sync", dir="horizontal",
          gap=8, cross="center"),
        F([icon("refresh", "subtle"), kbd("R")], name="refresh", dir="horizontal", gap=6,
          pad=[4, 6, 4, 6], r=6, cross="center"),
    ], name="topbar", dir="horizontal", gap=16, pad=[0, 16, 0, 24], w=1376, h=56, fill="base",
        stroke="hlLow", sides=(0, 0, 1, 0), cross="center", x=BOARD_X, y=0)


def screen(name, children, x):
    return F(children, name=name, w=1440, h=900, fill="base", clip=True, x=x, y=0)


# ---------------------------------------------------------------- issue panel

PANEL_X = 820
PANEL_W = 620
INNER = PANEL_W - 56
COMPOSER_H = 112
BODY_GAP = 12
COMMENTS_SCROLL = 525


def prop_row(name, value_children, active=False):
    return F([T(name, 12, "Medium", "muted", w=88),
              F(value_children, name="value", dir="horizontal", gap=10, cross="center",
                pad=[5, 8, 5, 8], r=6, fill="overlay" if active else None,
                stroke="hlMed" if active else None)],
             name=f"prop/{name}", dir="horizontal", cross="center", h=32)


def fake_admin(w, h, title, rows, hl, order="#4471"):
    """A mock web screenshot, drawn with fixed light colors because it is image content."""
    ink, soft, line, paper, bar = "#575279", "#797593", "#dfdad9", "#fffaf3", "#f2e9e1"
    trs = []
    for i, (a, b, c) in enumerate(rows):
        hot = i == hl
        trs.append(F([T(a, 11, "Medium", ink, MONO, w=64 if w < 400 else 90),
                      T(b, 11, "Regular", ink, MONO, fill_w=True),
                      T(c, 11, "SemiBold", "#b4637a" if hot else soft, MONO)],
                     name="row", dir="horizontal", gap=12, pad=[7, 12, 7, 12], cross="center",
                     sh="fill", fill="#b4637a1f" if hot else None,
                     stroke="#b4637a" if hot else line, sides=None if hot else (0, 0, 1, 0)))
    return F([
        F([E(8, 8, "#d7827e"), E(8, 8, "#ea9d34"), E(8, 8, "#56949f"),
           T(title, 11, "Medium", soft, extra={"opacity": 1})],
          name="browserbar", dir="horizontal", gap=6, pad=[8, 12, 8, 12], cross="center",
          sh="fill", fill=bar),
        F([T(f"Order {order}", 14, "SemiBold", ink), spacer(),
           F([T("Paid", 11, "SemiBold", "#286983")], name="status", dir="horizontal",
             pad=[2, 8, 2, 8], r=4, fill="#56949f33")],
          name="head", dir="horizontal", pad=[12, 12, 4, 12], cross="center", sh="fill"),
        F(trs, name="rows", dir="vertical", pad=[4, 12, 12, 12], sh="fill"),
    ], name="image", dir="vertical", w=w, h=h, r=6, fill=paper, clip=True, stroke="hlMed")


def comment(u, when, body, actions, children=(), image=None, edited=False):
    name = USERS[u][0]
    head = [T(name, 13, "SemiBold", "text"), T(when, 12, "Medium", "muted")]
    if edited:
        head.append(T("edited", 12, "Medium", "muted"))
    stack = [F(head, name="head", dir="horizontal", gap=8, cross="center"),
             T(body, 13, "Regular", "text", fill_w=True, lh=20)]
    if image:
        stack.append(image)
    stack.append(F([F([icon(a[1], "muted", 14), T(a[0], 12, "Medium", "muted")],
                      name=f"action/{a[0]}", dir="horizontal", gap=4, cross="center")
                    for a in actions], name="actions", dir="horizontal", gap=14))
    stack += list(children)
    return F([avatar(u, 24), F(stack, name="content", dir="vertical", gap=6, sh="fill")],
             name=f"comment/{u}", dir="horizontal", gap=10, sh="fill")


def panel_header():
    return F([T("WEB-214", 12, "Medium", "subtle", MONO), icon("chevron-right", "muted", 12),
              T("Webshop", 12, "Medium", "muted"), icon("chevron-right", "muted", 12),
              icon("board", "muted", 14), T("Webshop launch", 12, "Medium", "muted"),
              spacer(), icon_btn("copy"),
              icon_btn("external"), icon_btn("x"), kbd("Esc")],
             name="panel/header", dir="horizontal", gap=8, pad=[0, 16, 0, 28], w=PANEL_W, h=52,
             cross="center", stroke="hlLow", sides=(0, 0, 1, 0), x=0, y=0)


def prop_chip(name, children, active=False):
    return F(children, name=f"prop/{name}", dir="horizontal", gap=7, cross="center",
             pad=[0, 9, 0, 9], h=28, r=6, fill="overlay" if active else None,
             stroke="iris" if active else "hlMed")


def props_block(labels_active=False):
    return F([
        prop_chip("Status", [status_icon("progress"), T("In Progress", 12, "Medium", "text")]),
        prop_chip("Priority", [priority_icon("urgent"), T("Urgent", 12, "Medium", "text")]),
        prop_chip("Assignee", [avatar("KA", 18), T("Kaarel", 12, "Medium", "text")]),
        prop_chip("Labels", [label("Bug", 12), label("Backend", 12),
                             icon("plus", "muted", 14)], active=labels_active),
    ], name="properties", dir="horizontal", gap=8, wrap=True, sh="fill")


def section_head(title, right=()):
    return F([T(title, 13, "SemiBold", "subtle"), spacer(), *right], name=f"section/{title}",
             dir="horizontal", gap=8, cross="center", sh="fill")


DESC_P = ("When a customer gets a partial refund, the payment provider sends two webhooks. We handle both, "
          "and the second one flips the order back to paid.")


def description_view():
    return [
        section_head("Description", [btn("Edit", "ghost", "pencil"), kbd("E")]),
        T(DESC_P, 13, "Regular", "text", fill_w=True, lh=20),
        T("Steps", 14, "SemiBold", "text"),
        T("1.  Pay an order with a test card in the sandbox\n2.  Refund 20 EUR from the admin\n"
          "3.  After about 5 s the order shows Paid again", 13, "Regular", "text", fill_w=True,
          lh=22),
        F([fake_admin(INNER, 132, "admin.webshop.test/orders/4471",
                      [("14:02:11", "refund.created", "-20.00"),
                       ("14:02:16", "order.updated  status PAID", "PAID")], 1),
           T("admin-order-4471.png", 11, "Medium", "muted", MONO)],
          name="md/image", dir="vertical", gap=6, sh="fill"),
    ]


def comments_view():
    reply = F([comment("KA", "1 h ago",
                       "Then we ignore order.updated when the order already has a refund. "
                       "I will add a check in the webhook handler.",
                       [("Reply", "reply"), ("Edit", "pencil")], edited=True)],
              name="replies", dir="vertical", pad=[4, 0, 0, 14], stroke="hlMed", sw=2,
              sides=(0, 0, 0, 1), sh="fill")
    return [
        section_head("Comments", [T("3", 12, "Medium", "muted")]),
        comment("MT", "2 h ago",
                "Confirmed in the sandbox. The second webhook has status PAID because the payment provider "
                "sends the order snapshot, not the refund state.",
                [("Reply", "reply")], children=[reply]),
        comment("JK", "40 min ago", "Same thing with partial refunds on parcel orders:",
                [("Reply", "reply")],
                image=fake_admin(300, 128, "orders/4502",
                                 [("09:12:03", "refund.created", "-4.50"),
                                  ("09:12:08", "order.updated", "PAID")], 1, "#4502")),
    ]


def composer(draft=False):
    if draft:
        thumb = F([R(44, 30, "#fffaf3", r=3), R(30, 3, "#dfdad9", x=7, y=9),
                   R(22, 3, "#b4637a", x=7, y=17)], name="thumb", w=44, h=30, r=3, clip=True)
        inner = [T("Pushed the fix. A refund now keeps the order in Refunded:", 13, "Regular",
                   "text", fill_w=True, lh=20),
                 F([thumb, F([T("paste-0951.png", 12, "Medium", "subtle", MONO),
                              F([R(48, 3, "foam", r=2)], name="progress", w=120, h=3, r=2,
                                fill="hlMed", clip=True)], name="meta", dir="vertical", gap=6),
                    T("Uploading 40%", 11, "Medium", "muted"), icon_btn("x", "muted")],
                   name="attachment", dir="horizontal", gap=10, cross="center", pad=[6, 6, 6, 6],
                   r=6, fill="surface", stroke="hlLow")]
        box = F(inner, name="input", dir="vertical", gap=10, pad=12, r=8, fill="base",
                stroke="iris", sh="fill")
    else:
        box = F([T("Leave a comment. Markdown works. Paste an image to attach it.", 13,
                   "Regular", "muted", fill_w=True)], name="input", dir="vertical", pad=12, r=8,
                fill="base", stroke="hlMed", sh="fill", h=44)
    h = composer_h(draft)
    return F([
        box,
        F([icon("image", "muted", 14), T("Attach image", 12, "Medium", "muted"), spacer(),
           btn("Comment", "primary", k="Ctrl Enter")],
          name="row", dir="horizontal", gap=8, cross="center", sh="fill"),
    ], name="panel/composer", dir="vertical", gap=10, pad=[14, 28, 16, 28], w=PANEL_W,
        fill="surface", stroke="hlLow", sides=(1, 0, 0, 0), x=0, y=844 - h, h=h)


def composer_h(draft):
    return 186 if draft else COMPOSER_H


def panel(body_children, with_composer=True, scroll=0, draft=False):
    body_h = 844 - 52 - (composer_h(draft) if with_composer else 0)
    content = F(body_children, name="panel/content", dir="vertical", gap=BODY_GAP,
                pad=[20, 28, 24, 28], w=PANEL_W, x=0, y=-scroll)
    body = F([content], name="panel/body", w=PANEL_W, h=body_h, clip=True, x=0, y=52)
    ch = [panel_header(), body]
    if with_composer:
        ch.append(composer(draft))
    return F(ch, name="issue-panel", w=PANEL_W, h=844, fill="surface", stroke="hlMed",
             sides=(0, 0, 0, 1), fx=SHADOW, clip=True, x=PANEL_X, y=56)


def title_block(editing=False):
    t = T("Refund webhook marks the order paid twice", 20, "SemiBold", "text",
          fill_w=True, lh=28)
    return t


def labels_picker(x, y):
    rows = []
    checked = {"Bug", "Backend"}
    for name in ["Bug", "Backend", "Frontend", "Feature", "Improvement", "Design"]:
        hover = name == "Frontend"
        rows.append(F([checkbox(name in checked), E(8, 8, LABELS[name]),
                       T(name, 13, "Medium", "text"), spacer()],
                      name=f"option/{name}", dir="horizontal", gap=10, pad=[7, 10, 7, 10], r=6,
                      cross="center", sh="fill", fill="hlMed" if hover else None))
    return F([
        F([T("Filter labels", 13, "Regular", "muted"), spacer(), kbd("L")], name="filter",
          dir="horizontal", pad=[10, 12, 10, 12], cross="center", sh="fill", stroke="hlMed",
          sides=(0, 0, 1, 0)),
        F(rows, name="options", dir="vertical", gap=1, pad=4, sh="fill"),
        F([T("Enter toggles. Esc closes.", 11, "Medium", "muted")], name="hint",
          dir="horizontal", pad=[8, 12, 10, 12], sh="fill", stroke="hlMed", sides=(1, 0, 0, 0)),
    ], name="popover/labels", dir="vertical", w=260, r=8, fill="overlay", stroke="hlMed",
        fx=SHADOW, x=x, y=y, clip=True)


def md_editor():
    def ln(s, c="text"):
        return T(s, 12, "Regular", c, MONO, lh=20, name="line")

    lines = [
        ln("When a customer gets a partial refund, the payment"),
        ln("provider sends two webhooks. We handle both, and the"),
        ln("second one flips the order back to paid."),
        ln(""),
        ln("### Steps", "iris"),
        ln("1. Pay an order with a test card in the sandbox"),
        ln("2. Refund 20 EUR from the admin"),
        ln("3. After about 5 s the order shows Paid again"),
        ln(""),
        ln("![admin-order-4471.png](https://uploads.linear.app/...)", "muted"),
        F([ln("![Uploading paste-0942.png...]()", "gold"),
           R(2, 16, "iris", name="caret")], name="line/current", dir="horizontal", gap=1,
          cross="center"),
    ]
    tabs = F([
        F([T("Write", 12, "SemiBold", "text")], name="tab/write", dir="horizontal",
          pad=[6, 2, 6, 2], stroke="iris", sw=2, sides=(0, 0, 1, 0)),
        F([T("Preview", 12, "Medium", "subtle")], name="tab/preview", dir="horizontal",
          pad=[6, 2, 6, 2]),
        kbd("Ctrl P"), spacer(), T("Markdown", 11, "Medium", "muted"),
    ], name="tabs", dir="horizontal", gap=16, cross="center", sh="fill")
    upload = F([icon("image", "subtle", 14), T("paste-0942.png", 12, "Medium", "subtle", MONO),
                F([R(160, 4, "foam", r=2, name="bar")], name="progress", w=260, h=4, r=2,
                  fill="hlMed", clip=True),
                T("Uploading 62%", 12, "Medium", "muted")],
               name="upload", dir="horizontal", gap=10, cross="center", sh="fill",
               pad=[8, 10, 8, 10], r=6, fill="hlLow")
    return [
        section_head("Description", [T("Editing", 12, "Medium", "gold")]),
        tabs,
        F(lines, name="md/editor", dir="vertical", pad=[10, 12, 12, 12], r=6, fill="base",
          stroke="iris", sh="fill"),
        upload,
        F([T("Paste or drop images. They upload to Linear.", 12, "Medium", "muted"), spacer(),
           btn("Cancel", "ghost"), btn("Save", "primary", k="Ctrl S")],
          name="actions", dir="horizontal", gap=8, cross="center", sh="fill"),
    ]


# ---------------------------------------------------------------- development

WT_NAME = "refund-webhook"


def dev_section():
    d = DEV["WEB-214"]
    c = STAGE_COLOR[d["stage"]]
    return [
        section_head("Development", [T("set by Claude, 2 h ago", 11, "Medium", "muted")]),
        F([E(6, 6, c), T(d["stage"], 12, "SemiBold", c), icon("branch", "muted", 14),
           T(WT_NAME, 12, "Medium", "text", MONO), spacer(), icon_btn("copy", "muted")],
          name="dev", dir="horizontal", gap=8, cross="center", pad=[4, 4, 4, 12], r=8,
          fill="base", stroke="hlLow", sh="fill"),
    ]


# ---------------------------------------------------------------- screens

def s1(x):
    return screen("01 Board, grouped by team", [
        board([lane("Webshop", "WEB", WEB), lane("Platform", "PLT", PLT),
               lane("Design", "DES", {s: [I("x", "x", "low", None)] * n for (s, _), n in
                                      zip(STATES, [3, 2, 1, 0, 1])}, collapsed=True)],
              [WEB, PLT]),
        topbar("Team"), rail("acme"),
        F([T("northwind", 12, "SemiBold", "text"), T("1 team", 12, "Medium", "muted"),
           kbd("Alt 2")], name="tooltip/workspace", dir="horizontal", gap=8, cross="center",
          pad=[6, 10, 6, 10], r=6, fill="overlay", stroke="hlMed", fx=SHADOW_SM, x=58, y=93),
    ], x)


def s2(x):
    dragged = dict(WEB["In Progress"][0])
    # In Review column x inside the screen.
    cx = BOARD_X + BOARD_PAD + 3 * (COLW + COLGAP)
    return screen("02 Board, grouped by project, mine, dragging", [
        board([lane("Webshop launch", None, P_WEB, drop_col="In Review"),
               lane("Infra", None, P_INFRA), lane("No project", None, P_NONE)],
              [P_WEB, P_INFRA, P_NONE]),
        topbar("Project", mine=True, teams="Kaarel, 9 issues"),
        rail("acme"),
        card(dragged, w=COLW, fx=SHADOW, extra={"rotation": -2},
             selected=True) | {"x": cx + 10, "y": 142},
    ], x)


def s3(x):
    web = {k: [dict(i, _selected=(i["id"] == "WEB-214")) for i in v] for k, v in WEB.items()}
    return screen("03 Issue panel", [
        board([lane("Webshop", "WEB", web), lane("Platform", "PLT", PLT)], [web, PLT]),
        topbar("Team"), rail("acme"),
        panel([title_block(), props_block(), *dev_section(), *description_view(),
               *comments_view()]),
    ], x)


def s3b(x):
    web = {k: [dict(i, _selected=(i["id"] == "WEB-214")) for i in v] for k, v in WEB.items()}
    return screen("04 Issue panel, comments", [
        board([lane("Webshop", "WEB", web), lane("Platform", "PLT", PLT)], [web, PLT]),
        topbar("Team"), rail("acme"),
        panel([title_block(), props_block(), *dev_section(), *description_view(),
               *comments_view()], scroll=COMMENTS_SCROLL, draft=True),
    ], x)


def s4(x):
    web = {k: [dict(i, _selected=(i["id"] == "WEB-214")) for i in v] for k, v in WEB.items()}
    return screen("05 Issue panel, editing", [
        board([lane("Webshop", "WEB", web), lane("Platform", "PLT", PLT)], [web, PLT]),
        topbar("Team"), rail("acme"),
        panel([title_block(), props_block(labels_active=True), *dev_section(), *md_editor()],
              with_composer=False),
        # Measured from the built file: the Labels chip starts at x 1151.
        labels_picker(1151, 56 + 52 + 20 + 28 + BODY_GAP + 28 + 6),
    ], x)


def field(label_text, inner, w=None, focus=False, hint=None):
    ch = [T(label_text, 12, "SemiBold", "subtle"),
          F(inner, name="input", dir="horizontal", gap=8, pad=[9, 12, 9, 12], r=6, fill="base",
            stroke="iris" if focus else "hlMed", cross="center", sh="fill")]
    if hint:
        ch.append(T(hint, 11, "Medium", "muted"))
    return F(ch, name=f"field/{label_text}", dir="vertical", gap=6, w=w,
             sh=None if w else "fill")


def ws_row(letter, name, accent, key, meta):
    return F([ws_tile(letter, accent, active=None, s=32),
              F([T(name, 14, "SemiBold", "text"), T(meta, 12, "Medium", "muted")],
                name="text", dir="vertical", gap=2),
              spacer(), kbd(key), btn("Remove", "ghost")],
             name=f"ws/{name}", dir="horizontal", gap=12, pad=[10, 12, 10, 12], cross="center",
             sh="fill", stroke="hlLow", sides=(0, 0, 1, 0))


def s5(x):
    W = 600
    swatches = []
    for c in ["iris", "foam", "gold", "rose", "pine", "love"]:
        on = c == "gold"
        swatches.append(F([R(20, 20, c, r=4)], name=f"swatch/{c}", dir="horizontal", pad=3,
                          r=6, stroke="text" if on else None, sw=1.5))
    form = F([
        T("Add a workspace", 20, "SemiBold", "text"),
        T("In Linear, open Settings, Security and access, and create a personal API key. "
          "Paste it here. The app keeps the key in the system keyring, not in a file.",
          13, "Regular", "subtle", fill_w=True, lh=20),
        F([field("API key", [icon("key", "muted", 14),
                             T("lin_api_7Qm2••••••••"
                               "••••••Xk2Q", 13, "Medium", "text",
                               MONO), spacer(), kbd("Ctrl V")], focus=True),
           ], name="row", dir="horizontal", gap=8, sh="fill", cross="max"),
        F([F([icon("check", "foam", 14), T("Key works", 13, "SemiBold", "foam")],
             name="title", dir="horizontal", gap=6, cross="center"),
           T("Workspace bluebird. Signed in as Kaarel. 3 teams: Shop, Service, IT.", 12,
             "Medium", "subtle", fill_w=True)],
          name="check", dir="vertical", gap=4, pad=[10, 12, 10, 12], r=6, fill="surface",
          stroke="foam", sw=2, sides=(0, 0, 0, 1), sh="fill"),
        F([field("Name", [T("bluebird", 13, "Medium", "text")]),
           field("Shortcut", [kbd("Alt 3"), T("Press keys to change", 12, "Medium", "muted")],
                 w=220)], name="row", dir="horizontal", gap=16, sh="fill"),
        F([T("Color", 12, "SemiBold", "subtle"),
           F(swatches, name="swatches", dir="horizontal", gap=8)],
          name="field/Color", dir="vertical", gap=6),
        F([spacer(), btn("Cancel", "ghost", k="Esc"), btn("Add workspace", "primary",
                                                          k="Ctrl Enter")],
          name="actions", dir="horizontal", gap=8, cross="center", sh="fill"),
    ], name="add-form", dir="vertical", gap=20, pad=24, r=10, fill="surface", stroke="hlLow",
        sh="fill")
    content = F([
        F([T("Workspaces", 13, "SemiBold", "subtle"),
           F([ws_row("A", "acme", "iris", "Alt 1", "2 teams, 148 issues cached"),
              ws_row("N", "northwind", "foam", "Alt 2", "1 team, 61 issues cached")],
             name="list", dir="vertical", r=10, fill="surface", stroke="hlLow", sh="fill",
             clip=True)],
          name="existing", dir="vertical", gap=10, sh="fill"),
        form,
    ], name="content", dir="vertical", gap=28, w=W, x=BOARD_X + (1376 - W) // 2, y=56 + 40)
    bar = F([T("Workspaces", 15, "SemiBold", "text"), spacer(),
             btn("Back to board", "ghost", k="Esc")],
            name="topbar", dir="horizontal", gap=16, pad=[0, 16, 0, 24], w=1376, h=56,
            fill="base", stroke="hlLow", sides=(0, 0, 1, 0), cross="center", x=BOARD_X, y=0)
    return screen("06 Add workspace", [bar, content, rail(active=None, count=2, adding=True)], x)


def s6(x):
    sw = []
    for name, hexv in THEMES["main"].items():
        sw.append(F([R(150, 56, name, r=6, stroke="hlMed"),
                     T(name, 12, "SemiBold", "text"), T(hexv, 11, "Medium", "muted", MONO)],
                    name=f"token/{name}", dir="vertical", gap=4))
    type_rows = [
        T("Montserrat SemiBold 20. Issue title", 20, "SemiBold"),
        T("Montserrat SemiBold 13. Lane and section heads", 13, "SemiBold"),
        T("Montserrat Medium 13. Card title", 13, "Medium"),
        T("Montserrat Regular 13. Description and comments", 13, "Regular"),
        T("Montserrat Medium 12. Controls and meta", 12, "Medium", "subtle"),
        T("JetBrains Mono Medium 11. WEB-214, keys, code", 11, "Medium", "subtle", MONO),
    ]

    def group(title, items):
        return F([T(title, 12, "SemiBold", "muted"),
                  F(items, name="items", dir="horizontal", gap=18, cross="center", wrap=True,
                    w=640)],
                 name=f"group/{title}", dir="vertical", gap=10)

    def named(n, label_text):
        return F([n, T(label_text, 11, "Medium", "muted")], name="named", dir="horizontal",
                 gap=6, cross="center")

    comps = F([
        group("Status", [named(status_icon(k), s) for s, k in STATES] +
              [named(status_icon("canceled"), "Canceled")]),
        group("Priority", [named(priority_icon(p), PRIORITY_NAME[p]) for p in
                           ["urgent", "high", "medium", "low", "none"]]),
        group("Labels and people", [label(l) for l in LABELS] +
              [avatar(u) for u in USERS] + [avatar(None)]),
        group("Controls", [btn("Save", "primary", k="Ctrl S"), btn("Paste", "secondary"),
                           btn("Cancel", "ghost"), kbd("Alt 1"), checkbox(True),
                           checkbox(False), segmented(["Team", "Project"], "Team")]),
        group("Development", [F([dev_row(DEV[k])], name="dev", dir="horizontal", w=180)
                              for k in ["WEB-214", "WEB-210", "WEB-201", "WEB-208"]]),
        group("Card", [card(WEB["In Progress"][0], w=COLW),
                       card(WEB["In Progress"][0], selected=True, w=COLW)]),
    ], name="components", dir="vertical", gap=24, w=640)

    return screen("07 Theme tokens and parts", [
        F([
            F([T("Theme tokens", 20, "SemiBold"),
               T("Rosé Pine main. Every color is a variable in the theme collection, with "
                 "modes main, moon and dawn. Code reads the same token names.", 13, "Regular",
                 "subtle", w=560, lh=20)], name="head", dir="vertical", gap=6),
            F(sw, name="swatches", dir="horizontal", gap=16, wrap=True, w=660),
            F(type_rows, name="type", dir="vertical", gap=12),
        ], name="left", dir="vertical", gap=32, x=48, y=48, w=680),
        F([comps], name="right", dir="vertical", x=760, y=48),
    ], x)


# ---------------------------------------------------------------- emit

def emit(n, parent, out):
    ref = f"n{len(out)}"
    params = {"type": n["type"], "name": n["name"], "select": False}
    if parent:
        params["parent"] = {"$ref": parent}
    for k in ("x", "y", "width", "height", "text", "path"):
        if k in n:
            params[k] = n[k]
    params["props"] = n["props"]
    out.append({"command": "node.create", "as": ref, "params": params})
    for c in n["children"]:
        emit(c, ref, out)


def main():
    cmds = [{"command": "variable.createCollection", "as": "theme",
             "params": {"name": "theme", "modes": list(THEMES)}}]
    for name in THEMES["main"]:
        cmds.append({"command": "variable.create", "params": {
            "collection": {"$ref": "theme"}, "name": name, "type": "color",
            "values": {m: THEMES[m][name] for m in THEMES}}})
    cmds.append({"command": "page.setBackground", "params": {"color": "#0f0e17"}})
    for i, fn in enumerate([s1, s2, s3, s3b, s4, s5, s6]):
        emit(fn(i * 1540), None, cmds)
    json.dump([{"command": "command.batch", "params": {"label": "build", "commands": cmds}}],
              __import__("sys").stdout)


if __name__ == "__main__":
    main()
