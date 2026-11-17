#!/usr/bin/env python3
"""Design v3: one group at a time (tabs), Backlog as a strip, wider and more colorful cards.

Usage: python3 build_v3.py > script.json  (see build_v3.sh)
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import build as b  # noqa: E402
import build_v2 as v2  # noqa: E402
from build import (E, F, MONO, R, T, LABELS, STAGE_COLOR, WEB, PLT, avatar, icon, kbd,  # noqa: E402
                   spacer, status_icon, vspacer, trunc)
from build_v2 import a, RAIL_W, CARD_SHADOW, checkbox_accent  # noqa: E402

PAD = 24
GAP = 16
STRIP_W = 44
CONTENT_W = 1440 - RAIL_W
COLW = (CONTENT_W - 2 * PAD - STRIP_W - GAP - 3 * GAP) // 4  # 303
TOP_H = 60
TABS_H = 48
HEAD_H = 44
BOARD_Y = TOP_H + TABS_H

PRIO_COLOR = {"urgent": "love", "high": "gold", "medium": "foam", "low": "subtle", "none": "muted"}


def prio(p, s=14):
    """Priority glyph in the priority's own color."""
    c = PRIO_COLOR[p]
    if p == "urgent":
        return F([T("!", 10, "Bold", "base")], name="priority/urgent", dir="horizontal", w=s, h=s,
                 r=4, fill="love", main="center", cross="center")
    if p == "none":
        return F([R(3, 1.5, "muted", r=1, x=1, y=6.25), R(3, 1.5, "muted", r=1, x=5.5, y=6.25),
                  R(3, 1.5, "muted", r=1, x=10, y=6.25)], name="priority/none", w=s, h=s)
    n = {"high": 3, "medium": 2, "low": 1}[p]
    bars = [R(3, bh, c if i < n else "hlHigh", r=1, x=1 + i * 4.5, y=12.5 - bh)
            for i, bh in enumerate([5, 8, 11])]
    return F(bars, name=f"priority/{p}", w=s, h=s)


def tag(text, color, mono=False, ic=None):
    ch = []
    if ic:
        ch.append(icon(ic, color, 12))
    # Pine is too dark as text on a dark card. Its tag keeps the pine tint and uses foam text.
    ch.append(T(text, 11, "SemiBold", "foam" if color == "pine" else color,
                MONO if mono else b.UI))
    return F(ch, name=f"tag/{text}", dir="horizontal", gap=5, cross="center", pad=[3, 8, 3, 8],
             r=5, fill=a(color, 0x26))


def card(it, accent, selected=False, w=None, fx=None, extra=None):
    top_items = [prio(it["prio"]), T(it["id"], 12, "Medium", "subtle", MONO), spacer()]
    if it["comments"]:
        top_items.append(F([icon("comment", "muted", 14),
                            T(str(it["comments"]), 12, "Medium", "muted")],
                           name="comments", dir="horizontal", gap=4, cross="center"))
    top_items.append(avatar(it["user"], 24))
    top = F(top_items, name="top", dir="horizontal", gap=8, cross="center", sh="fill")
    title = T(it["title"], 14, "SemiBold" if selected else "Medium", "text", fill_w=True, lh=21)
    tags = [tag(l, LABELS[l]) for l in it["labels"]]
    d = it.get("dev")
    if d:
        c = STAGE_COLOR[d["stage"]]
        text = d["stage"] + (f"  {d['ref']}" if d.get("ref") else "")
        tags.append(tag(text, c, ic="branch" if d["stage"] in ("Local", "Merged") else "mr"))
    rows = [top, title]
    if tags:
        rows.append(F(tags, name="tags", dir="horizontal", gap=6, cross="center", sh="fill",
                      wrap=True))
    return F(rows, name=f"card/{it['id']}", dir="vertical", gap=10, pad=[14, 16, 14, 16],
             r=10, fill="overlay", stroke=accent if selected else "hlMed",
             sw=2 if selected else 1, fx=fx or CARD_SHADOW, w=w, sh=None if w else "fill",
             extra=extra)


def column(items, accent, h, slot=False, selected=None):
    ch = []
    for it in items:
        if it == "GHOST":
            ch.append(F([], name="drag/origin", h=150, r=10, fill=a("base", 0x80),
                        stroke="hlHigh", extra={"dashes": [4, 4], "sizingH": "fill"}))
        elif it == "SLOT":
            ch.append(F([], name="drag/slot", h=150, r=10, fill=a(accent, 0x14), stroke=accent,
                        sw=1.5, extra={"dashes": [5, 4], "sizingH": "fill"}))
        else:
            ch.append(card(it, accent, selected=selected == it["id"]))
    return F(ch, name="column", dir="vertical", gap=10, pad=10, w=COLW, h=h, r=14,
             fill=a(accent, 0x0d) if slot else "surface")


def strip(count, h):
    word = T("Backlog", 12, "SemiBold", "subtle", extra={"rotation": -90})
    return F([
        F([status_icon("backlog", 15), T(str(count), 12, "Medium", "muted", MONO)],
          name="head", dir="vertical", gap=8, cross="center"),
        F([word], name="label", w=20, h=70),
        vspacer(),
        icon("chevron-right", "muted", 14),
    ], name="backlog-strip", dir="vertical", gap=14, pad=[14, 0, 14, 0], w=STRIP_W, h=h, r=14,
        fill="surface", cross="center")


STATES4 = [s for s in b.STATES if s[0] != "Backlog"]
STATE_RULE = v2.STATE_RULE


def headers(data):
    hs = [F(name="strip-space", w=STRIP_W, h=1)]
    for s, kind in STATES4:
        n = sum(1 for i in data.get(s, []) if isinstance(i, dict))
        hs.append(F([
            F([status_icon(kind, 16), T(s, 14, "SemiBold", "text"),
               T(str(n), 12, "Medium", "muted", MONO), spacer(),
               icon("plus", "muted", 14) if False else spacer()],
              name="label", dir="horizontal", gap=9, cross="center", pad=[0, 6, 0, 6], sh="fill"),
            R(COLW, 3, STATE_RULE[kind], r=2, name="state-rule", extra={"sizingH": "fill"}),
        ], name=f"colheader/{s}", dir="vertical", gap=10, w=COLW))
    return F(hs, name="column-headers", dir="horizontal", gap=GAP, pad=[0, PAD, 0, PAD],
             w=CONTENT_W, h=HEAD_H, cross="max", x=RAIL_W, y=BOARD_Y)


def board(data, accent, slot_col=None, selected=None, col_h=None):
    col_h = col_h or (900 - BOARD_Y - HEAD_H - 16 - 20)
    backlog = sum(1 for i in data.get("Backlog", []) if isinstance(i, dict))
    cols = [strip(backlog, col_h)] + [
        column(data.get(s, []), accent, col_h, slot=(s == slot_col), selected=selected)
        for s, _ in STATES4]
    return F(cols, name="board", dir="horizontal", gap=GAP, pad=[0, PAD, 0, PAD], w=CONTENT_W,
             x=RAIL_W, y=BOARD_Y + HEAD_H + 16)


def topbar(ws, sub, accent, mine=False):
    mine_box = F([checkbox_accent(mine, accent),
                  T("Assigned to me", 12, "SemiBold", "text" if mine else "subtle"), kbd("M")],
                 name="filter/mine", dir="horizontal", gap=8, cross="center", pad=[6, 10, 6, 10],
                 r=8, fill="surface" if mine else None)
    return F([
        F([T(ws, 20, "Bold", "text"), T(sub, 12, "Medium", "muted")], name="title",
          dir="horizontal", gap=10, cross="max"),
        spacer(), mine_box,
        F([E(7, 7, "foam"), T("Synced 12 s ago", 12, "Medium", "muted")], name="sync",
          dir="horizontal", gap=8, cross="center"),
        F([icon("refresh", "subtle", 15)], name="refresh", dir="horizontal", pad=7, r=8,
          fill="surface"),
    ], name="topbar", dir="horizontal", gap=14, pad=[0, 24, 0, PAD], w=CONTENT_W, h=TOP_H,
        cross="center", x=RAIL_W, y=0)


def tabs(group, items, active, accent):
    """The group switch and one tab per team or project. Only the active one shows below."""
    ch = [v2.seg(["Team", "Project"], group, accent), R(1, 20, "hlMed", name="divider")]
    for name, color, count in items:
        on = name == active
        inner = []
        if color:
            inner.append(R(10, 10, color, r=3, name="color"))
        inner += [T(name, 13, "SemiBold", "text" if on else "subtle"),
                  T(str(count), 11, "Medium", accent if on else "muted", MONO)]
        tab = F([F(inner, name="label", dir="horizontal", gap=8, cross="center",
                   pad=[0, 4, 0, 4]),
                 R(10, 2, accent if on else "base", r=1, name="underline",
                   extra={"sizingH": "fill"})],
                name=f"tab/{name}", dir="vertical", gap=12, pad=[14, 0, 0, 0])
        ch.append(tab)
    return F(ch, name="group-tabs", dir="horizontal", gap=20, pad=[0, PAD, 0, PAD], w=CONTENT_W,
             h=TABS_H, cross="center", stroke="hlLow", sides=(0, 0, 1, 0), x=RAIL_W, y=TOP_H)


TEAM_TABS = [("All teams", None, 23), ("Webshop", "iris", 10), ("Platform", "foam", 6),
             ("Design", "rose", 7)]
PROJECT_TABS = [("All projects", None, 6), ("Back office", "foam", 6), ("Reports", "gold", 0)]


def s1(x):
    return v2.screen("v3 01 Board, acme", [
        topbar("acme", "3 teams", "iris"),
        tabs("Team", TEAM_TABS, "Webshop", "iris"),
        headers(WEB),
        board(WEB, "iris"),
        v2.rail("acme", "iris"),
    ], x)


def s2(x):
    data = v2.NW_DATA
    cx = RAIL_W + PAD + STRIP_W + GAP + 2 * (COLW + GAP)
    dragged = card(v2.NW_DRAG, "foam", selected=True, w=COLW - 20,
                   fx=[{"type": "dropShadow", "color": "#000000a0", "offset": [0, 18],
                        "radius": 36}], extra={"rotation": -2})
    dragged["x"] = cx + 22
    dragged["y"] = BOARD_Y + HEAD_H + 16 + 4
    return v2.screen("v3 02 Board, northwind, dragging", [
        topbar("northwind", "Kaarel, 6 issues", "foam", mine=True),
        tabs("Project", PROJECT_TABS, "Back office", "foam"),
        headers(data),
        board(data, "foam", slot_col="In Review"),
        v2.rail("northwind", "foam"),
        dragged,
    ], x)


def s3(x):
    return v2.screen("v3 03 Issue panel", [
        topbar("acme", "3 teams", "iris"),
        tabs("Team", TEAM_TABS, "Webshop", "iris"),
        headers(WEB),
        board(WEB, "iris", selected="WEB-214"),
        v2.rail("acme", "iris"),
        F(name="scrim", w=v2.PANEL_X - RAIL_W, h=900, fill=a("base", 0x66), x=RAIL_W, y=0),
        panel(),
    ], x)


def panel():
    """The v2 panel, with labels drawn as the same tinted tags as the cards."""
    old = v2.b.label
    v2.b.label = lambda name, size=11: tag(name, LABELS[name])
    try:
        return v2.panel_v2("iris")
    finally:
        v2.b.label = old



# ------------------------------------------------------------------ v3b additions

from build import USERS, DEV, fake_admin  # noqa: E402

EXTRA_TODO = [
    b.I("WEB-236", "Show tyre load index next to the size", "low", "MT", ["Frontend"]),
    b.I("WEB-237", "Email reminder one day before the fitting slot", "medium", "LS",
        ["Feature"], 1),
    b.I("WEB-238", "Admin: filter bookings by fitting station", "low", "AP", ["Improvement"]),
]


def menu_item(ic, text, k=None, hover=False, danger=False):
    ch = [icon(ic, "text" if hover else "subtle", 15),
          T(text, 13, "Medium", "text" if hover else "subtle"), spacer()]
    if k:
        ch.append(T(k, 11, "Medium", "muted", MONO))
    return F(ch, name=f"menu/{text}", dir="horizontal", gap=10, cross="center",
             pad=[8, 10, 8, 10], r=7, fill="hlMed" if hover else None, sh="fill")


def context_menu(x, y):
    return F([
        menu_item("external", "Open", "Enter"),
        menu_item("copy", "Copy ID and title", "Ctrl C", hover=True),
        menu_item("asterisk", "Copy prompt for Claude", "Ctrl Shift C"),
        R(10, 1, "hlMed", name="divider", extra={"sizingH": "fill"}),
        menu_item("link", "Open in Linear"),
    ], name="context-menu", dir="vertical", gap=2, pad=6, w=290, r=12, fill="overlay",
        stroke="hlMed", fx=b.SHADOW, x=x, y=y)


def toast(text, x, y):
    return F([icon("check", "foam", 15), T("Copied", 13, "SemiBold", "text"),
              T(text, 12, "Medium", "subtle", MONO)],
             name="toast", dir="horizontal", gap=10, cross="center", pad=[11, 16, 11, 14], r=10,
             fill="overlay", stroke="hlMed", fx=b.SHADOW_SM, x=x, y=y)


def scroll_column(items, accent, h):
    """A column whose cards overflow: it clips, fades at the bottom and shows its own bar."""
    inner = F([card(it, accent) for it in items], name="cards", dir="vertical", gap=10,
              w=COLW - 20, x=10, y=10)
    fade = R(COLW, 64, None, name="fade", x=0, y=h - 64,
             extra={"fills": [{"type": "linearGradient", "stops": [b.M if False else
                                                                     v2.M["surface"] + "00",
                                                                     v2.M["surface"]],
                               "from": [0.5, 0], "to": [0.5, 1]}], "cornerRadius": [0, 0, 14, 14]})
    bar = R(4, 160, "hlHigh", r=2, name="scrollbar", x=COLW - 7, y=60)
    return F([inner, fade, bar], name="column", w=COLW, h=h, r=14, fill="surface", clip=True)


def board_scroll(data, accent, selected=None):
    col_h = 900 - BOARD_Y - HEAD_H - 16 - 20
    backlog = sum(1 for i in data.get("Backlog", []) if isinstance(i, dict))
    cols = [strip(backlog, col_h)]
    for st, _ in STATES4:
        items = data.get(st, [])
        if st == "Todo":
            cols.append(scroll_column(items + EXTRA_TODO, accent, col_h))
        else:
            cols.append(column(items, accent, col_h, selected=selected))
    return F(cols, name="board", dir="horizontal", gap=GAP, pad=[0, PAD, 0, PAD], w=CONTENT_W,
             x=RAIL_W, y=BOARD_Y + HEAD_H + 16)


def head_btn(ic, text, k=None):
    ch = [icon(ic, "subtle", 14), T(text, 12, "SemiBold", "subtle")]
    return F(ch, name=f"head/{text}", dir="horizontal", gap=6, cross="center",
             pad=[6, 10, 6, 10], r=8, fill="overlay")


PANEL_W = v2.PANEL_W
PANEL_X = v2.PANEL_X


def prop(label, value):
    return F([T(label, 12, "Medium", "muted", w=104), F(value, name="value", dir="horizontal",
                                                          gap=8, cross="center")],
             name=f"prop/{label}", dir="horizontal", cross="center", h=34)


def tool(ic=None, text=None, on=False, mono=False):
    ch = []
    if ic:
        ch.append(icon(ic, "text" if on else "subtle", 15))
    if text:
        ch.append(T(text, 13, "Bold", "text" if on else "subtle", MONO if mono else b.UI))
    return F(ch, name=f"tool/{text or ic}", dir="horizontal", main="center", cross="center",
             w=30, h=28, r=6, fill="hlMed" if on else None)


def styled_line(parts, size=14, lh=24):
    """One editor line built from (text, weight, color, mono) runs, so markdown marks look dimmed."""
    runs = []
    for txt, wt, c, mono in parts:
        # Text frames drop spaces at their edges, so edge spaces become no-break spaces.
        txt = txt.replace(" ", "\u00a0") if txt.strip() != txt else txt
        runs.append(T(txt, size if not mono else size - 1, wt, c, MONO if mono else b.UI))
    return F(runs, name="line", dir="horizontal", cross="max", h=lh)


def editor():
    toolbar = F([
        tool(text="H"), tool(text="B", on=True), tool(text="</>", mono=True),
        R(1, 18, "hlMed", name="sep"),
        tool("board"), tool(text="1."), tool("check"),
        R(1, 18, "hlMed", name="sep"),
        tool("link"), tool("image"),
        spacer(),
        v2.seg(["Write", "Preview"], "Write", "iris"),
    ], name="toolbar", dir="horizontal", gap=4, cross="center", pad=[6, 8, 6, 8], sh="fill",
        stroke="hlLow", sides=(0, 0, 1, 0))
    mk = "muted"
    lines = [
        styled_line([("When a customer gets a ", "Regular", "text", False),
                     ("**", "Regular", mk, True), ("partial refund", "Bold", "text", False),
                     ("**", "Regular", mk, True), (", the payment provider sends", "Regular", "text", False)]),
        styled_line([("two webhooks. We handle both, and the second one flips the order back", "Regular", "text", False)]),
        styled_line([("to paid.", "Regular", "text", False)]),
        F(name="blank", w=10, h=10),
        styled_line([("## ", "Medium", mk, True), ("Steps", "Bold", "text", False)], size=17, lh=30),
        styled_line([("1. ", "Medium", "iris", True), ("Pay an order with a test card in the sandbox", "Regular", "text", False)]),
        styled_line([("2. ", "Medium", "iris", True), ("Refund 20 EUR from the admin", "Regular", "text", False)]),
        styled_line([("3. ", "Medium", "iris", True), ("After about 5 s the order shows Paid again", "Regular", "text", False)]),
        F(name="blank", w=10, h=10),
        styled_line([("- [ ] ", "Medium", "iris", True), ("Ignore ", "Regular", "text", False),
                     ("`order.updated`", "Regular", "foam", True),
                     (" when a refund exists", "Regular", "text", False)]),
        F([styled_line([("- [x] ", "Medium", "iris", True),
                        ("Add a test for partial refunds", "Regular", "subtle", False)]),
           R(1.5, 20, "iris", name="caret")], name="line/current", dir="horizontal", gap=2,
          cross="center"),
    ]
    body = F(lines, name="body", dir="vertical", gap=2, pad=[14, 16, 16, 16], sh="fill")

    def thumb(name, uploading=False):
        img = F([R(64, 40, "#fffaf3", r=4), R(40, 4, "#dfdad9", x=8, y=10),
                 R(28, 4, "#b4637a", x=8, y=22)], name="thumb", w=64, h=40, r=5, clip=True)
        meta = [T(name, 12, "Medium", "text", MONO)]
        if uploading:
            meta.append(F([R(70, 3, "iris", r=2)], name="progress", w=110, h=3, r=2,
                          fill="hlMed", clip=True))
        else:
            meta.append(T("Image, 84 KB", 11, "Medium", "muted"))
        return F([img, F(meta, name="meta", dir="vertical", gap=6), spacer(),
                  icon("x", "muted", 14)], name="attachment", dir="horizontal", gap=12,
                 cross="center", pad=8, r=10, fill="overlay", w=270)

    attachments = F([thumb("admin-order-4471.png"), thumb("paste-0942.png", uploading=True)],
                    name="attachments", dir="horizontal", gap=10, pad=[0, 12, 12, 12], wrap=True,
                    sh="fill")
    return F([toolbar, body, attachments], name="editor", dir="vertical", r=12, fill="base",
             stroke="iris", sw=1.5, sh="fill", clip=True)


def panel_v3(mode="view"):
    issue = WEB["In Progress"][0]
    d = DEV["WEB-214"]
    sc = STAGE_COLOR[d["stage"]]
    header = F([
        T("WEB-214", 12, "Medium", "subtle", MONO), T("in", 12, "Medium", "muted"),
        T("Webshop", 12, "Medium", "subtle"), spacer(),
        head_btn("copy", "ID and title"), head_btn("asterisk", "Prompt"),
        b.icon_btn("external", "muted"), b.icon_btn("x", "muted"),
    ], name="panel/header", dir="horizontal", gap=6, cross="center", pad=[0, 18, 0, 36],
        w=PANEL_W, h=60)
    title = T(issue["title"], 22, "SemiBold", "text", fill_w=True, lh=30)
    props = F([
        prop("Status", [status_icon("progress", 15), T("In Progress", 13, "Medium", "text")]),
        prop("Priority", [prio("urgent"), T("Urgent", 13, "Medium", "text")]),
        prop("Assignee", [avatar("KA", 20), T("Kaarel", 13, "Medium", "text")]),
        prop("Labels", [tag("Bug", "love"), tag("Backend", "pine"), icon("plus", "muted", 14)]),
        prop("Development", [tag(f"Local  {d['ref']}", sc, ic="branch"),
                             T("set by Claude, 2 h ago", 12, "Medium", "muted")]),
    ], name="properties", dir="vertical", sh="fill")
    rule = R(PANEL_W - 72, 1, "hlLow", name="rule", extra={"sizingH": "fill"})
    if mode == "edit":
        desc = [
            F([T("Description", 14, "SemiBold", "text"), spacer(), b.btn("Cancel", "ghost", k="Esc"),
               v2.primary("Save", "iris", "Ctrl S")], name="section/Description",
              dir="horizontal", gap=8, cross="center", sh="fill"),
            editor(),
            T("Ctrl B bold, Ctrl K link, Ctrl Shift 7 list. Paste or drop images to attach them.",
              11, "Medium", "muted"),
        ]
    else:
        desc = [
            F([T("Description", 14, "SemiBold", "text"), spacer(), b.btn("Edit", "ghost", "pencil"),
               kbd("E")], name="section/Description", dir="horizontal", gap=6, cross="center",
              sh="fill"),
            T(b.DESC_P, 14, "Regular", "text", w=560, lh=23),
            T("Steps", 16, "Bold", "text"),
            T("1.  Pay an order with a test card in the sandbox\n2.  Refund 20 EUR from the admin\n"
              "3.  After about 5 s the order shows Paid again", 14, "Regular", "text", w=560, lh=24),
            F([fake_admin(PANEL_W - 72, 140, "admin.webshop.test/orders/4471",
                          [("14:02:11", "refund.created", "-20.00"),
                           ("14:02:16", "order.updated  status PAID", "PAID")], 1)],
              name="md/image", dir="vertical", sh="fill"),
        ]
    body = F([title, props, rule, *desc], name="panel/content", dir="vertical", gap=18,
             pad=[8, 36, 24, 36], w=PANEL_W, x=0, y=60)
    composer = F([
        F([avatar("KA", 26),
           F([T("Write a comment. Markdown works, and pasted images upload.", 13, "Regular",
                "muted", fill_w=True)], name="input", dir="vertical", pad=[11, 14, 11, 14], r=10,
             fill="base", stroke="hlMed", sh="fill"),
           v2.primary("Comment", "iris", "Ctrl Enter")],
          name="row", dir="horizontal", gap=12, cross="center", sh="fill"),
    ], name="panel/composer", dir="vertical", pad=[14, 36, 16, 36], w=PANEL_W, h=74,
        fill="surface", x=0, y=900 - 74)
    ch = [body, header] + ([] if mode == "edit" else [composer])
    return F(ch, name="issue-panel", w=PANEL_W, h=900, fill="surface", fx=v2.PANEL_SHADOW,
             clip=True, x=PANEL_X, y=0)


def s1b(x):
    web = {k: list(v) for k, v in WEB.items()}
    menu_x = RAIL_W + PAD + STRIP_W + GAP + COLW + GAP + 150
    return v2.screen("v3 01 Board, acme, copy menu", [
        topbar("acme", "3 teams", "iris"),
        tabs("Team", TEAM_TABS, "Webshop", "iris"),
        headers({**web, "Todo": web["Todo"] + EXTRA_TODO}),
        board_scroll(web, "iris", selected="WEB-214"),
        v2.rail("acme", "iris"),
        context_menu(menu_x, 236),
        toast("WEB-214 Refund webhook marks the order paid twice", 470, 830),
    ], x)


def s3b(x):
    return v2.screen("v3 03 Issue panel", [
        topbar("acme", "3 teams", "iris"),
        tabs("Team", TEAM_TABS, "Webshop", "iris"),
        headers(WEB),
        board(WEB, "iris", selected="WEB-214"),
        v2.rail("acme", "iris"),
        F(name="scrim", w=PANEL_X - RAIL_W, h=900, fill=a("base", 0x66), x=RAIL_W, y=0),
        panel_v3("view"),
        toast("start WEB-214 in acme", PANEL_X + 190, 756),
    ], x)


def s5(x):
    return v2.screen("v3 05 Issue panel, editing", [
        topbar("acme", "3 teams", "iris"),
        tabs("Team", TEAM_TABS, "Webshop", "iris"),
        headers(WEB),
        board(WEB, "iris", selected="WEB-214"),
        v2.rail("acme", "iris"),
        F(name="scrim", w=PANEL_X - RAIL_W, h=900, fill=a("base", 0x66), x=RAIL_W, y=0),
        panel_v3("edit"),
    ], x)

def main():
    cmds = [{"command": "variable.createCollection", "as": "theme",
             "params": {"name": "theme", "modes": list(b.THEMES)}}]
    for name in b.THEMES["main"]:
        cmds.append({"command": "variable.create", "params": {
            "collection": {"$ref": "theme"}, "name": name, "type": "color",
            "values": {m: b.THEMES[m][name] for m in b.THEMES}}})
    cmds.append({"command": "page.setBackground", "params": {"color": "#0f0e17"}})
    screens = [s1b, s2, s3b, lambda x: dict(v2.s4(x), name="v3 04 Add workspace"), s5]
    for i, fn in enumerate(screens):
        b.emit(fn(i * 1540), None, cmds)
    json.dump([{"command": "command.batch", "params": {"label": "v3", "commands": cmds}}],
              sys.stdout)


if __name__ == "__main__":
    main()
