#!/usr/bin/env python3
"""Design v2: depth (base, wells, cards), per-workspace accent, panel as a document.

Usage: python3 build_v2.py > script.json  (see build_v2.sh)
Reuses the v1 helpers and mock data from build.py.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import build as b  # noqa: E402
from build import (E, F, I, MONO, R, T, USERS, LABELS, STAGE_COLOR, DEV, WEB, PLT, avatar,  # noqa: E402
                   icon, kbd, priority_icon, spacer, status_icon, vspacer, trunc, checkbox,
                   fake_admin)

M = b.THEMES["main"]
CARD_SHADOW = [{"type": "dropShadow", "color": "#0000004d", "offset": [0, 1], "radius": 2},
               {"type": "dropShadow", "color": "#00000026", "offset": [0, 4], "radius": 10}]
PANEL_SHADOW = [{"type": "dropShadow", "color": "#00000099", "offset": [-12, 0], "radius": 40}]


def a(tok, alpha):
    """A theme color with an alpha, for tints. alpha is 0..255."""
    return M[tok] + f"{alpha:02x}"


STATE_RULE = {"backlog": "muted", "todo": "subtle", "progress": "gold", "review": "foam",
              "done": "iris"}
RAIL_W = 72
COLW = 256
GAP = 12
PAD = 24
TOP_H = 64


# ------------------------------------------------------------------ rail and top bar

WORKSPACES = [("A", "acme", "iris", 1, "2 teams"), ("N", "northwind", "foam", 2, "1 team"),
              ("B", "bluebird", "gold", 3, "3 teams")]


def rail(active, accent, adding=False, count=3):
    items = []
    for letter, name, acc, n, _ in WORKSPACES[:count]:
        on = name == active
        tile = F([T(letter, 17, "Bold", acc)], name=f"tile/{name}", dir="horizontal", w=44, h=44,
                 r=12, fill=a(acc, 0x2e if on else 0x14), stroke=acc if on else None, sw=1.5,
                 main="center", cross="center")
        items.append(F([tile, T(f"Alt {n}", 9, "Medium", "subtle" if on else "muted", MONO)],
                       name=f"rail/{name}", dir="vertical", gap=6, cross="center"))
    add = F([icon("plus", accent if adding else "subtle", 16)], name="tile/add",
            dir="horizontal", w=44, h=44, r=12, fill=a(accent, 0x2e) if adding else None,
            stroke=accent if adding else "hlMed", sw=1.5 if adding else 1, main="center",
            cross="center", extra=None if adding else {"dashes": [3, 3]})
    col = F(items + [vspacer(), add], name="rail", dir="vertical", gap=16, pad=[18, 0, 18, 0],
            w=RAIL_W, h=900, fill="base", cross="center", x=0, y=0)
    ch = [col, R(1, 900, "hlLow", name="divider", x=RAIL_W - 1, y=0)]
    if not adding:
        idx = [w[1] for w in WORKSPACES].index(active)
        # Active marker: a short bar on the rail edge, in the workspace accent.
        ch.append(R(3, 24, accent, r=2, name="active-marker", x=0, y=18 + 10 + idx * (44 + 6 + 11 + 16)))
    return F(ch, name="rail", w=RAIL_W, h=900, x=0, y=0)


def seg(options, active, accent):
    items = []
    for o in options:
        on = o == active
        items.append(F([T(o, 12, "SemiBold", accent if on else "subtle")], name=f"seg/{o}",
                       dir="horizontal", pad=[5, 12, 5, 12], r=5, fill=a(accent, 0x24) if on else None))
    return F(items, name="segmented", dir="horizontal", pad=3, gap=2, r=8, fill="surface")


def topbar(ws, teams, accent, group="Team", mine=False, w=1440 - RAIL_W):
    mine_box = F([checkbox_accent(mine, accent),
                  T("Assigned to me", 12, "SemiBold", "text" if mine else "subtle"), kbd("M")],
                 name="filter/mine", dir="horizontal", gap=8, cross="center", pad=[6, 10, 6, 10],
                 r=8, fill="surface" if mine else None)
    return F([
        F([T(ws, 20, "Bold", "text"), T(teams, 12, "Medium", "muted")], name="title",
          dir="horizontal", gap=10, cross="max"),
        F(name="gap", w=12, h=1),
        seg(["Team", "Project"], group, accent),
        mine_box,
        spacer(),
        F([E(7, 7, "foam"), T("Synced 12 s ago", 12, "Medium", "muted")], name="sync",
          dir="horizontal", gap=8, cross="center"),
        F([icon("refresh", "subtle", 15)], name="refresh", dir="horizontal", pad=7, r=8,
          fill="surface"),
    ], name="topbar", dir="horizontal", gap=14, pad=[0, 24, 0, PAD], w=w, h=TOP_H,
        cross="center", x=RAIL_W, y=0)


def checkbox_accent(on, accent, s=15):
    if on:
        return F([b.node("vector", "check", {"stroke": b.col("base"), "strokeWeight": 1.8,
                                             "fill": "none", "strokeCap": "round",
                                             "strokeJoin": "round"},
                         path="M3.8 7.6 L6.4 10.2 L11.2 5", x=0, y=0)],
                 name="checkbox/on", w=s, h=s, r=4, fill=accent)
    return F(name="checkbox/off", w=s, h=s, r=4, stroke="hlHigh", sw=1.5)


# ------------------------------------------------------------------ board

def card(it, accent, selected=False, w=None, fx=None, extra=None):
    top = F([T(it["id"], 11, "Medium", "muted", MONO), spacer(), priority_icon(it["prio"])],
            name="top", dir="horizontal", cross="center", sh="fill")
    title = T(it["title"], 13, "SemiBold" if selected else "Medium", "text", fill_w=True, lh=19)
    rows = [top, title]
    d = it.get("dev")
    if d:
        c = STAGE_COLOR[d["stage"]]
        dev = [icon("branch" if d["stage"] in ("Local", "Merged") else "mr", c, 13),
               T(d["stage"], 11, "SemiBold", c)]
        if d.get("ref"):
            dev.append(trunc(d["ref"], 11, "Regular", "subtle", MONO))
        rows.append(F(dev, name="dev", dir="horizontal", gap=6, cross="center", sh="fill"))
    foot = [b.label(l) for l in it["labels"]]
    if it["comments"]:
        foot.append(F([icon("comment", "muted", 13), T(str(it["comments"]), 11, "Medium", "muted")],
                      name="comments", dir="horizontal", gap=3, cross="center"))
    foot += [spacer(), avatar(it["user"], 20)]
    rows.append(F(foot, name="foot", dir="horizontal", gap=10, cross="center", sh="fill"))
    return F(rows, name=f"card/{it['id']}", dir="vertical", gap=8, pad=[11, 12, 11, 12], r=8,
             fill="overlay", stroke=accent if selected else "hlMed", sw=1.5 if selected else 1,
             fx=fx or CARD_SHADOW, w=w, sh=None if w else "fill", extra=extra)


def well(items, accent, slot=None, ghost=False, selected=None, h=None):
    ch = []
    for it in items:
        if it == "GHOST":
            ch.append(F([], name="drag/origin", h=b.DRAG_H, r=8, fill=a("base", 0x80),
                        stroke="hlHigh", sw=1, extra={"dashes": [4, 4], "sizingH": "fill"}))
        elif it == "SLOT":
            ch.append(F([], name="drag/slot", h=b.DRAG_H, r=8, fill=a(accent, 0x14), stroke=accent,
                        sw=1.5, extra={"dashes": [5, 4], "sizingH": "fill"}))
        else:
            ch.append(card(it, accent, selected=selected == it["id"]))
    return F(ch, name="well", dir="vertical", gap=8, pad=8, w=COLW, h=h, r=12,
             fill=a(accent, 0x0d) if slot else "surface",
             extra={"minHeight": 52})


# Height of the tallest well per lane, measured in the built file. Wells in one lane share
# it, because figcraft cannot stretch hug children to the row height.
LANE_H = {"Webshop": 394, "Platform": 254, "Back office": 273}


def lane(title, key, color, data, accent, collapsed=False, slot_col=None, selected=None):
    n = sum(1 for s in data.values() for i in s if isinstance(i, dict) or i == "GHOST")
    head = [icon("chevron-right" if collapsed else "chevron-down", "muted", 14),
            R(10, 10, color, r=3, name="team-color"),
            T(title, 14, "SemiBold", "text")]
    if key:
        head.append(T(key, 11, "Medium", "muted", MONO))
    head += [T(str(n), 12, "Medium", "muted")]
    header = F(head, name="lane/header", dir="horizontal", gap=9, cross="center", h=34)
    ch = [header]
    if not collapsed:
        wells = [well(data.get(s, []), accent, slot=(s == slot_col), selected=selected,
                      h=LANE_H.get(title)) for s, _ in b.STATES]
        ch.append(F(wells, name="lane/wells", dir="horizontal", gap=GAP, cross="min"))
    return F(ch, name=f"lane/{title}", dir="vertical", gap=6)


def col_headers(groups):
    hs = []
    for s, kind in b.STATES:
        n = sum(1 for g in groups for i in g.get(s, []) if isinstance(i, dict))
        hs.append(F([
            F([status_icon(kind, 15), T(s, 13, "SemiBold", "text"), spacer(),
               T(str(n), 12, "Medium", "muted", MONO)], name="label", dir="horizontal", gap=9,
              cross="center", pad=[0, 4, 0, 4], sh="fill"),
            R(COLW, 2, STATE_RULE[kind], r=1, name="state-rule", extra={"sizingH": "fill", "opacity": 0.7}),
        ], name=f"colheader/{s}", dir="vertical", gap=10, w=COLW))
    return F(hs, name="column-headers", dir="horizontal", gap=GAP, pad=[0, PAD, 0, PAD],
             w=1440 - RAIL_W, cross="max", h=40)


def board(lanes, groups, h=900 - TOP_H):
    body = F(lanes, name="lanes", dir="vertical", gap=22, pad=[14, PAD, 24, PAD],
             w=1440 - RAIL_W)
    return F([col_headers(groups), body], name="board", dir="vertical", w=1440 - RAIL_W, h=h,
             fill="base", clip=True, x=RAIL_W, y=TOP_H)


def screen(name, children, x):
    return F(children, name=name, w=1440, h=900, fill="base", clip=True, x=x, y=0)


# ------------------------------------------------------------------ data for northwind

def NW(id, title, prio, user, labels=(), comments=0):
    return I(id, title, prio, user, labels, comments)


NW_DATA = {
    "Backlog": [NW("NW-412", "Import supplier price lists from CSV", "low", None, ["Feature"])],
    "Todo": [NW("NW-418", "Invoice PDF shows wrong VAT number", "high", "KA", ["Bug"], 2),
             NW("NW-415", "Stock count screen for the warehouse tablet", "medium", "KA",
                ["Feature", "Design"])],
    "In Progress": ["GHOST",
                    NW("NW-409", "Faster product search on the order form", "medium", "KA",
                       ["Improvement"], 1)],
    "In Review": ["SLOT"],
    "Done": [NW("NW-401", "Credit note export to accounting", "medium", "KA", ["Feature"], 3)],
}
NW_DRAG = NW("NW-420", "Sync customer balances from accounting every night", "urgent", "KA",
             ["Backend"], 4)
NW_DRAG["dev"] = dict(stage="MR", ref="!57")
NW_DATA["Todo"][0]["dev"] = dict(stage="Local", ref="invoice-vat-number")
NW_DATA["Done"][0]["dev"] = dict(stage="Prod")


# ------------------------------------------------------------------ issue panel

PANEL_W = 660
PANEL_X = 1440 - PANEL_W


def prop(label, value):
    return F([T(label, 12, "Medium", "muted", w=104), F(value, name="value", dir="horizontal",
                                                          gap=8, cross="center")],
             name=f"prop/{label}", dir="horizontal", cross="center", h=34)


def panel_v2(accent):
    issue = WEB["In Progress"][0]
    d = DEV["WEB-214"]
    sc = STAGE_COLOR[d["stage"]]
    header = F([
        T("WEB-214", 12, "Medium", "subtle", MONO), T("in", 12, "Medium", "muted"),
        T("Webshop", 12, "Medium", "subtle"), T("/", 12, "Medium", "muted"),
        T("Webshop launch", 12, "Medium", "subtle"), spacer(),
        b.icon_btn("copy", "muted"), b.icon_btn("external", "muted"), b.icon_btn("x", "muted"),
    ], name="panel/header", dir="horizontal", gap=6, cross="center", pad=[0, 18, 0, 36],
        w=PANEL_W, h=56)
    title = T(issue["title"], 22, "SemiBold", "text", fill_w=True, lh=30)
    props = F([
        prop("Status", [status_icon("progress", 15), T("In Progress", 13, "Medium", "text")]),
        prop("Priority", [priority_icon("urgent"), T("Urgent", 13, "Medium", "text")]),
        prop("Assignee", [avatar("KA", 20), T("Kaarel", 13, "Medium", "text")]),
        prop("Labels", [b.label("Bug", 12), b.label("Backend", 12),
                        icon("plus", "muted", 14)]),
        prop("Development", [icon("branch", sc, 14), T(d["stage"], 13, "SemiBold", sc),
                             T(d["ref"], 12, "Regular", "subtle", MONO),
                             T("set by Claude, 2 h ago", 12, "Medium", "muted")]),
    ], name="properties", dir="vertical", sh="fill")
    rule = R(PANEL_W - 72, 1, "hlLow", name="rule", extra={"sizingH": "fill"})
    desc = [
        F([T("Description", 14, "SemiBold", "text"), spacer(), b.btn("Edit", "ghost", "pencil"),
           kbd("E")], name="section/Description", dir="horizontal", gap=6, cross="center",
          sh="fill"),
        T(b.DESC_P, 14, "Regular", "text", w=560, lh=23),
        T("1.  Pay an order with a test card in the sandbox\n2.  Refund 20 EUR from the admin\n"
          "3.  After about 5 s the order shows Paid again", 14, "Regular", "text", w=560, lh=24),
        F([fake_admin(PANEL_W - 72, 140, "admin.webshop.test/orders/4471",
                      [("14:02:11", "refund.created", "-20.00"),
                       ("14:02:16", "order.updated  status PAID", "PAID")], 1)],
          name="md/image", dir="vertical", sh="fill"),
    ]

    def entry(u, when, body, edited=False, reply=False, image=None, own=False):
        name = USERS[u][0]
        head = [T(name, 13, "SemiBold", "text"), T(when, 12, "Medium", "muted")]
        if edited:
            head.append(T("edited", 12, "Medium", "muted"))
        stack = [F(head, name="head", dir="horizontal", gap=8, cross="center"),
                 T(body, 13, "Regular", "text", fill_w=True, lh=21)]
        if image:
            stack.append(image)
        acts = [F([icon("reply", "muted", 13), T("Reply", 12, "Medium", "muted")],
                  name="reply", dir="horizontal", gap=4, cross="center")]
        if own:
            acts.append(F([icon("pencil", "muted", 13), T("Edit", 12, "Medium", "muted")],
                          name="edit", dir="horizontal", gap=4, cross="center"))
        stack.append(F(acts, name="actions", dir="horizontal", gap=14))
        return F([avatar(u, 26), F(stack, name="content", dir="vertical", gap=6, sh="fill")],
                 name="entry", dir="horizontal", gap=12, sh="fill",
                 pad=[0, 0, 0, 30] if reply else 0)

    thread = F([
        entry("MT", "2 h ago", "Confirmed in the sandbox. The second webhook has status PAID "
              "because the payment provider sends the order snapshot, not the refund state."),
        entry("KA", "1 h ago", "Then we ignore order.updated when the order already has a "
              "refund. I will add a check in the webhook handler.", edited=True, reply=True,
              own=True),
    ], name="timeline", dir="vertical", gap=18, sh="fill")
    # The thread line joins the avatars of one conversation.
    activity = [
        F([T("Activity", 14, "SemiBold", "text"), T("3 comments", 12, "Medium", "muted")],
          name="section/Activity", dir="horizontal", gap=10, cross="center"),
        thread,
    ]
    body = F([title, props, rule, *desc, rule, *activity], name="panel/content",
             dir="vertical", gap=18, pad=[8, 36, 24, 36], w=PANEL_W, x=0, y=56)
    composer = F([
        F([avatar("KA", 26),
           F([T("Write a comment. Markdown works, and pasted images upload.", 13, "Regular",
                "muted", fill_w=True)], name="input", dir="vertical", pad=[11, 14, 11, 14], r=10,
             fill="base", stroke="hlMed", sh="fill"),
           b.btn("Comment", "primary", k="Ctrl Enter")],
          name="row", dir="horizontal", gap=12, cross="center", sh="fill"),
    ], name="panel/composer", dir="vertical", pad=[14, 36, 16, 36], w=PANEL_W, h=74,
        fill="surface", x=0, y=900 - 74)
    line = R(2, 70, "hlMed", r=1, name="thread-line", x=36 + 12, y=0)
    panel = F([body, header, composer], name="issue-panel", w=PANEL_W, h=900, fill="surface",
              fx=PANEL_SHADOW, clip=True, x=PANEL_X, y=0)
    return panel


def primary(text, accent, k=None):
    ch = [T(text, 12, "SemiBold", "base")]
    if k:
        ch.append(T(k, 10, "Medium", "base", MONO, extra={"opacity": 0.7}))
    return F(ch, name=f"button/{text}", dir="horizontal", gap=6, pad=[7, 14, 7, 14], r=7,
             fill=accent, cross="center")


# ------------------------------------------------------------------ screens

def s1(x):
    web = {k: list(v) for k, v in WEB.items()}
    return screen("v2 01 Board, acme", [
        board([lane("Webshop", "WEB", "iris", web, "iris"),
               lane("Platform", "PLT", "foam", PLT, "iris"),
               lane("Design", "DES", "rose", {}, "iris", collapsed=True)], [web, PLT]),
        topbar("acme", "3 teams", "iris"),
        rail("acme", "iris"),
    ], x)


def s2(x):
    cx = RAIL_W + PAD + 3 * (COLW + GAP)
    dragged = card(NW_DRAG, "foam", selected=True, w=COLW,
                   fx=[{"type": "dropShadow", "color": "#000000a0", "offset": [0, 18],
                        "radius": 36}], extra={"rotation": -2})
    dragged["x"] = cx + 14
    dragged["y"] = TOP_H + 40 + 14 + 34 + 6 + 2
    return screen("v2 02 Board, northwind, dragging", [
        board([lane("Back office", None, "foam", NW_DATA, "foam", slot_col="In Review")],
              [NW_DATA]),
        topbar("northwind", "Kaarel, 6 issues", "foam", group="Project", mine=True),
        rail("northwind", "foam"),
        dragged,
    ], x)


def s3(x):
    web = {k: list(v) for k, v in WEB.items()}
    return screen("v2 03 Issue panel", [
        board([lane("Webshop", "WEB", "iris", web, "iris", selected="WEB-214"),
               lane("Platform", "PLT", "foam", PLT, "iris")], [web, PLT]),
        topbar("acme", "3 teams", "iris"),
        rail("acme", "iris"),
        F(name="scrim", w=PANEL_X - RAIL_W, h=900, fill=a("base", 0x66), x=RAIL_W, y=0),
        panel_v2("iris"),
    ], x)


def s4(x):
    accent = "gold"
    steps = []
    for n, (title, text) in enumerate([
        ("Create a key", "In Linear, open Settings, then Security and access, then Personal API keys."),
        ("Paste it here", "The key stays on this computer, in a file only you can read."),
        ("Pick a key and a color", "The shortcut switches to this workspace from anywhere in the app."),
    ], 1):
        steps.append(F([
            F([T(str(n), 12, "Bold", accent if n < 3 else "subtle")], name="num",
              dir="horizontal", w=26, h=26, r=13, fill=a(accent, 0x24) if n < 3 else "overlay",
              main="center", cross="center"),
            F([T(title, 14, "SemiBold", "text"), T(text, 13, "Regular", "subtle", w=300, lh=20)],
              name="text", dir="vertical", gap=4),
        ], name=f"step/{n}", dir="horizontal", gap=14))
    left = F([
        T("Add a workspace", 28, "Bold", "text"),
        T("Each workspace is one Linear organization with its own key.", 14, "Regular",
          "subtle", w=340, lh=22),
        F(steps, name="steps", dir="vertical", gap=22),
    ], name="intro", dir="vertical", gap=24, w=360)
    key_field = F([
        T("API key", 12, "SemiBold", "subtle"),
        F([icon("key", accent, 15), T("lin_api_7Qm2•••••••"
                                      "•••••Xk2Q", 14, "Medium", "text",
                                      MONO), spacer(), kbd("Ctrl V")],
          name="input", dir="horizontal", gap=10, pad=[12, 14, 12, 14], r=9, fill="base",
          stroke=accent, sw=1.5, cross="center", sh="fill"),
        F([icon("check", "foam", 14),
           T("Works. bluebird, signed in as Kaarel, teams Shop, Service and IT.", 12,
             "Medium", "foam")], name="ok", dir="horizontal", gap=6, cross="center"),
    ], name="field/key", dir="vertical", gap=8, sh="fill")

    def field(label, inner, w=None):
        return F([T(label, 12, "SemiBold", "subtle"),
                  F(inner, name="input", dir="horizontal", gap=8, pad=[11, 14, 11, 14], r=9,
                    fill="base", stroke="hlMed", cross="center", sh="fill")],
                 name=f"field/{label}", dir="vertical", gap=8, w=w, sh=None if w else "fill")

    sw = []
    for c in ["iris", "foam", "gold", "rose", "pine", "love"]:
        on = c == accent
        sw.append(F([R(24, 24, c, r=7)], name=f"swatch/{c}", dir="horizontal", pad=3, r=10,
                    stroke=c if on else None, sw=2))
    preview = F([F([T("B", 17, "Bold", accent)], name="tile", dir="horizontal", w=44, h=44,
                   r=12, fill=a(accent, 0x2e), stroke=accent, sw=1.5, main="center",
                   cross="center"),
                 F([T("bluebird", 15, "SemiBold", "text"),
                    T("Alt 3 switches here", 12, "Medium", "muted")], name="text",
                   dir="vertical", gap=3)],
                name="preview", dir="horizontal", gap=14, cross="center", pad=14, r=12,
                fill="base")
    form = F([
        key_field,
        F([field("Name", [T("bluebird", 14, "Medium", "text")]),
           field("Shortcut", [kbd("Alt 3"), T("Press keys", 12, "Medium", "muted")], w=200)],
          name="row", dir="horizontal", gap=16, sh="fill"),
        F([T("Color", 12, "SemiBold", "subtle"), F(sw, name="swatches", dir="horizontal", gap=6)],
          name="field/Color", dir="vertical", gap=8),
        preview,
        F([spacer(), b.btn("Cancel", "ghost", k="Esc"), primary("Add workspace", accent,
                                                                "Ctrl Enter")],
          name="actions", dir="horizontal", gap=10, cross="center", sh="fill"),
    ], name="form", dir="vertical", gap=24, pad=28, w=560, r=16, fill="surface")
    content = F([left, form], name="content", dir="horizontal", gap=72,
                x=RAIL_W + (1440 - RAIL_W - 992) // 2, y=150)
    back = F([b.btn("Back to board", "ghost", k="Esc")], name="back", dir="horizontal",
             x=1440 - 24 - 160, y=16)
    return screen("v2 04 Add workspace", [content, back, rail(None, accent, adding=True)], x)


def main():
    cmds = [{"command": "variable.createCollection", "as": "theme",
             "params": {"name": "theme", "modes": list(b.THEMES)}}]
    for name in b.THEMES["main"]:
        cmds.append({"command": "variable.create", "params": {
            "collection": {"$ref": "theme"}, "name": name, "type": "color",
            "values": {m: b.THEMES[m][name] for m in b.THEMES}}})
    cmds.append({"command": "page.setBackground", "params": {"color": "#0f0e17"}})
    for i, fn in enumerate([s1, s2, s3, s4]):
        b.emit(fn(i * 1540), None, cmds)
    json.dump([{"command": "command.batch", "params": {"label": "v2", "commands": cmds}}],
              sys.stdout)


if __name__ == "__main__":
    main()
