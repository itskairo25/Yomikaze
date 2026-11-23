# builds the yomikaze icon font from hand drawn geometry on a 24 unit grid
import math, sys, os
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

UPM = 1024
SCALE = UPM / 24.0
ASC, DESC = 960, -64
W = 1.75  # stroke width
BASE = 0xF0000

# geometry in svg style coordinates (y down)

def circle_pts(cx, cy, r, n=48, a0=0.0, a1=360.0):
    out = []
    for i in range(n + 1):
        a = math.radians(a0 + (a1 - a0) * i / n)
        out.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return out

def capsule(a, b, w):
    (x1, y1), (x2, y2) = a, b
    dx, dy = x2 - x1, y2 - y1
    ang = math.atan2(dy, dx)
    r = w / 2
    pts = []
    for i in range(9):
        t = ang - math.pi / 2 + math.pi * i / 8
        pts.append((x2 + r * math.cos(t), y2 + r * math.sin(t)))
    for i in range(9):
        t = ang + math.pi / 2 + math.pi * i / 8
        pts.append((x1 + r * math.cos(t), y1 + r * math.sin(t)))
    return pts

def disc(cx, cy, r, n=40):
    return circle_pts(cx, cy, r, n)[:-1]

def rounded(points, rc, closed):
    # replaces each corner with a quadratic curve
    if rc <= 0 or len(points) < 3:
        return list(points) + ([points[0]] if closed else [])
    pts = list(points)
    n = len(pts)
    out = []
    idx = range(n) if closed else range(1, n - 1)
    if not closed:
        out.append(pts[0])
    for i in idx:
        p0, p1, p2 = pts[i - 1], pts[i], pts[(i + 1) % n]
        d1 = math.dist(p0, p1); d2 = math.dist(p1, p2)
        c = min(rc, d1 / 2, d2 / 2)
        a = (p1[0] + (p0[0] - p1[0]) * c / d1, p1[1] + (p0[1] - p1[1]) * c / d1)
        b = (p1[0] + (p2[0] - p1[0]) * c / d2, p1[1] + (p2[1] - p1[1]) * c / d2)
        for k in range(9):
            t = k / 8
            x = (1 - t) ** 2 * a[0] + 2 * (1 - t) * t * p1[0] + t * t * b[0]
            y = (1 - t) ** 2 * a[1] + 2 * (1 - t) * t * p1[1] + t * t * b[1]
            out.append((x, y))
    if closed:
        out.append(out[0])
    else:
        out.append(pts[-1])
    return out

def spline(points, closed=False, steps=10):
    # catmull rom through the given points
    pts = list(points)
    if closed:
        ext = [pts[-1]] + pts + [pts[0], pts[1]]
        count = len(pts)
    else:
        ext = [pts[0]] + pts + [pts[-1]]
        count = len(pts) - 1
    out = []
    for i in range(count):
        p0, p1, p2, p3 = ext[i], ext[i + 1], ext[i + 2], ext[i + 3]
        for s in range(steps):
            t = s / steps
            t2, t3 = t * t, t * t * t
            x = 0.5 * (2 * p1[0] + (-p0[0] + p2[0]) * t + (2 * p0[0] - 5 * p1[0] + 4 * p2[0] - p3[0]) * t2 + (-p0[0] + 3 * p1[0] - 3 * p2[0] + p3[0]) * t3)
            y = 0.5 * (2 * p1[1] + (-p0[1] + p2[1]) * t + (2 * p0[1] - 5 * p1[1] + 4 * p2[1] - p3[1]) * t2 + (-p0[1] + 3 * p1[1] - 3 * p2[1] + p3[1]) * t3)
            out.append((x, y))
    out.append(pts[0] if closed else pts[-1])
    return out

def rrect_pts(x, y, w, h, r):
    return rounded([(x, y), (x + w, y), (x + w, y + h), (x, y + h)], r, True)

def rotate(points, cx, cy, deg):
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    return [(cx + (px - cx) * c - (py - cy) * s, cy + (px - cx) * s + (py - cy) * c) for px, py in points]

def mirror(points):
    return [(24 - x, y) for x, y in points]


class Icon:
    def __init__(self):
        self.ink = []    # filled contours
        self.holes = []  # cut out contours

    def stroke(self, pts, w=W):
        pts = list(pts)
        for a, b in zip(pts, pts[1:]):
            if math.dist(a, b) < 1e-6:
                continue
            self.ink.append(capsule(a, b, w))
        return self

    def line(self, *pts, w=W):
        return self.stroke(pts, w)

    def path(self, pts, rc=1.6, closed=False, w=W):
        return self.stroke(rounded(pts, rc, closed), w)

    def ring(self, cx, cy, r, w=W):
        self.ink.append(disc(cx, cy, r + w / 2, 64))
        self.holes.append(disc(cx, cy, r - w / 2, 64))
        return self

    def arc(self, cx, cy, r, a0, a1, w=W, head=None):
        pts = circle_pts(cx, cy, r, max(8, int(abs(a1 - a0) / 6)), a0, a1)
        self.stroke(pts, w)
        if head:
            self.head(pts[-2], pts[-1], head, w)
        return self

    def head(self, before, tip, size=3.2, w=W, spread=42):
        ang = math.atan2(before[1] - tip[1], before[0] - tip[0])
        for s in (-1, 1):
            t = ang + math.radians(spread) * s
            self.line(tip, (tip[0] + size * math.cos(t), tip[1] + size * math.sin(t)), w=w)
        return self

    def rect(self, x, y, w_, h, r, w=W):
        outer = rrect_pts(x - w / 2, y - w / 2, w_ + w, h + w, r + w / 2)
        inner = rrect_pts(x + w / 2, y + w / 2, w_ - w, h - w, max(r - w / 2, 0.01))
        self.ink.append(outer[:-1])
        self.holes.append(inner[:-1])
        return self

    def fill(self, pts):
        self.ink.append(list(pts))
        return self

    def dot(self, cx, cy, r=1.2):
        self.ink.append(disc(cx, cy, r))
        return self


def area(pts):
    s = 0
    for (x1, y1), (x2, y2) in zip(pts, pts[1:] + pts[:1]):
        s += x1 * y2 - x2 * y1
    return s / 2

def to_font(pts):
    return [(round(x * SCALE), round(ASC - y * SCALE)) for x, y in pts]

def dedupe(pts):
    out = []
    for p in pts:
        if not out or p != out[-1]:
            out.append(p)
    if len(out) > 1 and out[0] == out[-1]:
        out.pop()
    return out

def glyph(icon):
    pen = TTGlyphPen(None)
    for contours, clockwise in ((icon.ink, True), (icon.holes, False)):
        for c in contours:
            pts = dedupe(to_font(c))
            if len(pts) < 3:
                continue
            a = area(pts)  # y up: clockwise is negative
            if (a > 0) == clockwise:
                pts.reverse()
            pen.moveTo(pts[0])
            for p in pts[1:]:
                pen.lineTo(p)
            pen.closePath()
    return pen.glyph()


# the icons

ICONS = {}

def icon(name):
    def deco(fn):
        i = Icon()
        fn(i)
        ICONS[name] = i
        return fn
    return deco

@icon("BOOKS")
def _(i):
    i.rect(3.25, 3.75, 5, 16.25, 1.4)
    i.rect(10, 6.25, 4.75, 13.75, 1.4)
    book = rrect_pts(17.25, 4.5, 4.5, 14.75, 1.4)
    book = rotate(book, 21.75, 19.25, -15)
    lowest = max(p[1] for p in book)
    book = [(x + 0.3, y - (lowest - 20.0)) for x, y in book]
    i.stroke(book)

@icon("COMPASS")
def _(i):
    i.ring(12, 12, 8.75)
    i.fill([(15.9, 8.1), (13.4, 13.4), (8.1, 15.9), (10.6, 10.6)])

@icon("MAGNIFYING_GLASS")
def _(i):
    i.ring(10.75, 10.75, 6.5)
    i.line((15.5, 15.5), (20.25, 20.25), w=2.1)

@icon("CLOCK_COUNTER_CLOCKWISE")
def _(i):
    i.arc(12, 12, 8.5, 205, 520)
    i.line((3.6, 5.6), (3.6, 9.4), (7.4, 9.4))
    i.line((12, 7.5), (12, 12), (15, 14))

@icon("CLOCK")
def _(i):
    i.ring(12, 12, 8.75)
    i.line((12, 7.25), (12, 12), (15.25, 14))

@icon("GEAR_SIX")
def _(i):
    pts = []
    for k in range(6):
        a = k * 60
        for d, r in ((-24, 7.0), (-12, 9.3), (12, 9.3), (24, 7.0)):
            t = math.radians(a + d)
            pts.append((12 + r * math.cos(t), 12 + r * math.sin(t)))
    i.path(pts, rc=0.9, closed=True)
    i.ring(12, 12, 2.9)

@icon("X")
def _(i):
    i.line((6.25, 6.25), (17.75, 17.75))
    i.line((17.75, 6.25), (6.25, 17.75))

@icon("PLUS")
def _(i):
    i.line((12, 5), (12, 19))
    i.line((5, 12), (19, 12))

@icon("CHECK")
def _(i):
    i.line((4.75, 12.75), (9.5, 17.5), (19.25, 7.25))

@icon("CHECK_FAT")
def _(i):
    i.line((4.75, 12.75), (9.5, 17.5), (19.25, 7.25), w=3.0)

@icon("CHECK_CIRCLE")
def _(i):
    i.ring(12, 12, 8.75)
    i.line((8.1, 12.4), (10.75, 15), (15.9, 9.4))

@icon("X_CIRCLE")
def _(i):
    i.ring(12, 12, 8.75)
    i.line((9.2, 9.2), (14.8, 14.8))
    i.line((14.8, 9.2), (9.2, 14.8))

@icon("WARNING_CIRCLE")
def _(i):
    i.ring(12, 12, 8.75)
    i.line((12, 7.5), (12, 12.75))
    i.dot(12, 16.1, 1.2)

@icon("INFO")
def _(i):
    i.ring(12, 12, 8.75)
    i.line((12, 11), (12, 16.25))
    i.dot(12, 7.9, 1.2)

@icon("WARNING")
def _(i):
    i.path([(12, 3.75), (21, 19.75), (3, 19.75)], rc=2.2, closed=True)
    i.line((12, 9.5), (12, 13.75))
    i.dot(12, 16.6, 1.15)

@icon("DOWNLOAD_SIMPLE")
def _(i):
    i.line((12, 3.75), (12, 14.25))
    i.line((7.75, 10), (12, 14.25), (16.25, 10))
    i.path([(4, 14.5), (4, 20), (20, 20), (20, 14.5)], rc=2.2)

@icon("DOWNLOAD")
def _(i):
    i.line((12, 3.75), (12, 14.25))
    i.line((7.75, 10), (12, 14.25), (16.25, 10))
    i.path([(4, 14.5), (4, 20), (20, 20), (20, 14.5)], rc=2.2)

@icon("ARROW_CIRCLE_DOWN")
def _(i):
    i.ring(12, 12, 8.75)
    i.line((12, 7.5), (12, 16))
    i.line((8.6, 12.6), (12, 16), (15.4, 12.6))

@icon("CARET_CIRCLE_RIGHT")
def _(i):
    i.ring(12, 12, 8.75)
    i.line((10.5, 8), (14.5, 12), (10.5, 16))

@icon("TRASH")
def _(i):
    i.line((3.75, 6.5), (20.25, 6.5))
    i.path([(9, 6.25), (9, 3.75), (15, 3.75), (15, 6.25)], rc=1.2)
    i.path([(5.9, 6.75), (6.9, 20.25), (17.1, 20.25), (18.1, 6.75)], rc=1.8)
    i.line((10, 10.5), (10, 16.25))
    i.line((14, 10.5), (14, 16.25))

@icon("WIFI_SLASH")
def _(i):
    i.arc(12, 19, 12.0, 228, 312)
    i.arc(12, 19, 8.0, 230, 310)
    i.arc(12, 19, 4.0, 235, 305)
    i.dot(12, 19, 1.35)
    i.line((4, 3.5), (20, 20.5))

@icon("WIFI_X")
def _(i):
    i.arc(12, 19, 12.0, 228, 312)
    i.arc(12, 19, 8.0, 230, 310)
    i.arc(12, 19, 4.0, 235, 305)
    i.dot(12, 19, 1.35)
    i.line((4, 3.5), (20, 20.5))

@icon("ARROWS_CLOCKWISE")
def _(i):
    i.arc(12, 12, 7.75, 200, 335, head=3.0)
    i.arc(12, 12, 7.75, 20, 155, head=3.0)

@icon("BOOKMARK_SIMPLE")
def _(i):
    i.path([(6.5, 3.75), (17.5, 3.75), (17.5, 20.25), (12, 16.25), (6.5, 20.25)], rc=1.8, closed=True)

@icon("BOOKMARK_FILL")
def _(i):
    shape = rounded([(6.5, 3.75), (17.5, 3.75), (17.5, 20.25), (12, 16.25), (6.5, 20.25)], 1.8, True)
    i.fill(shape[:-1])
    i.stroke(shape)

def heart_pts(scale=0.52, cy=10.9):
    pts = []
    for k in range(120):
        t = 2 * math.pi * k / 120
        x = 16 * math.sin(t) ** 3
        y = 13 * math.cos(t) - 5 * math.cos(2 * t) - 2 * math.cos(3 * t) - math.cos(4 * t)
        pts.append((12 + x * scale, cy - y * scale))
    return pts

@icon("HEART")
def _(i):
    p = heart_pts()
    i.stroke(p + [p[0]])

@icon("HEART_FILL")
def _(i):
    p = heart_pts()
    i.fill(p)
    i.stroke(p + [p[0]])

@icon("PLAY")
def _(i):
    tri = rounded([(7.5, 4.75), (19, 12), (7.5, 19.25)], 2.0, True)
    i.fill(tri[:-1])
    i.stroke(tri, w=1.2)

def star_pts(cx=12, cy=12.6, ro=9.0, ri=4.1):
    pts = []
    for k in range(10):
        a = math.radians(-90 + 36 * k)
        r = ro if k % 2 == 0 else ri
        pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts

@icon("STAR")
def _(i):
    i.path(star_pts(), rc=1.0, closed=True)

@icon("STAR_FILL")
def _(i):
    p = rounded(star_pts(), 1.0, True)
    i.fill(p[:-1])
    i.stroke(p)

def sparkle_pts(cx, cy, a, n=0.62):
    pts = []
    e = 2 / n
    for k in range(96):
        t = 2 * math.pi * k / 96
        c, s = math.cos(t), math.sin(t)
        pts.append((cx + a * math.copysign(abs(c) ** e, c), cy + a * math.copysign(abs(s) ** e, s)))
    return pts

@icon("SPARKLE")
def _(i):
    p = sparkle_pts(10.5, 13, 8.0)
    i.stroke(p + [p[0]])
    q = sparkle_pts(18.5, 5.5, 3.0)
    i.fill(q)

@icon("FIRE")
def _(i):
    outer = [(12, 3.25), (15.2, 6.9), (18.2, 10.9), (18.6, 15.2), (16.4, 19.1), (12, 20.75), (7.6, 19.1), (5.4, 15.2), (5.9, 11.2), (8.2, 8.4), (9.4, 11.2)]
    i.stroke(spline(outer, closed=True))
    inner = [(12, 12.4), (14.3, 15.4), (13.9, 17.9), (12, 18.9), (10.1, 17.9), (9.7, 15.4)]
    i.stroke(spline(inner, closed=True))

@icon("FOLDER")
def _(i):
    i.path([(3.5, 19), (3.5, 5.25), (9.25, 5.25), (11.25, 7.75), (20.5, 7.75), (20.5, 19)], rc=1.9, closed=True)

@icon("FOLDER_OPEN")
def _(i):
    i.path([(3.5, 19), (3.5, 5.25), (9.25, 5.25), (11.25, 7.75), (18, 7.75), (18, 10.5)], rc=1.6)
    i.path([(3.5, 19), (6.5, 10.75), (21.5, 10.75), (18.5, 19)], rc=1.6, closed=True)

@icon("DOTS_THREE")
def _(i):
    for x in (5.5, 12, 18.5):
        i.dot(x, 12, 1.6)

@icon("SORT_ASCENDING")
def _(i):
    i.line((3.75, 7), (11.5, 7))
    i.line((3.75, 12), (9.75, 12))
    i.line((3.75, 17), (8, 17))
    i.line((17, 19), (17, 5))
    i.line((13.75, 8.25), (17, 5), (20.25, 8.25))

@icon("FUNNEL_SIMPLE")
def _(i):
    i.line((4, 7), (20, 7))
    i.line((7, 12), (17, 12))
    i.line((10, 17), (14, 17))

@icon("LOCK_KEY")
def _(i):
    i.rect(4.75, 10.5, 14.5, 10, 2.6)
    i.line((8.25, 10.25), (8.25, 7.75))
    i.arc(12, 7.75, 3.75, 180, 360)
    i.line((15.75, 7.75), (15.75, 10.25))
    i.dot(12, 15.5, 1.4)

@icon("LOCK_KEY_OPEN")
def _(i):
    i.rect(4.75, 10.5, 14.5, 10, 2.6)
    i.line((8.25, 10.25), (8.25, 7.25))
    i.arc(12, 7.25, 3.75, 180, 340)
    i.dot(12, 15.5, 1.4)

@icon("IMAGE")
def _(i):
    i.rect(3.5, 4.75, 17, 14.5, 2.6)
    i.path([(4, 16.75), (8.75, 12), (12.5, 15.75)], rc=0.8)
    i.path([(11, 14.25), (14.25, 11), (20, 16.75)], rc=0.8)
    i.dot(15.6, 8.6, 1.45)

@icon("SUN")
def _(i):
    i.ring(12, 12, 4.1)
    for k in range(8):
        a = math.radians(k * 45)
        i.line((12 + 7.1 * math.cos(a), 12 + 7.1 * math.sin(a)), (12 + 9.2 * math.cos(a), 12 + 9.2 * math.sin(a)))

def crescent(cx, cy, r, ox, oy, ro):
    outer = [p for p in circle_pts(cx, cy, r, 180) if math.dist(p, (ox, oy)) >= ro]
    inner = [p for p in circle_pts(ox, oy, ro, 180) if math.dist(p, (cx, cy)) <= r]
    # start the outer run right after the cut so the points are in order
    full = circle_pts(cx, cy, r, 180)
    start = next(k for k in range(len(full)) if math.dist(full[k], (ox, oy)) < ro and math.dist(full[(k + 1) % len(full)], (ox, oy)) >= ro)
    ordered = [full[(start + 1 + k) % len(full)] for k in range(len(full))]
    outer = [p for p in ordered if math.dist(p, (ox, oy)) >= ro]
    fullin = circle_pts(ox, oy, ro, 180)
    s2 = next(k for k in range(len(fullin)) if math.dist(fullin[k], (cx, cy)) > r and math.dist(fullin[(k + 1) % len(fullin)], (cx, cy)) <= r)
    ordin = [fullin[(s2 + 1 + k) % len(fullin)] for k in range(len(fullin))]
    inner = [p for p in ordin if math.dist(p, (cx, cy)) <= r]
    # outer runs one way, the inner arc comes back the other way
    a = outer
    b = inner
    if math.dist(a[-1], b[0]) > math.dist(a[-1], b[-1]):
        b = b[::-1]
    return a + b + [a[0]]

@icon("MOON")
def _(i):
    i.stroke(crescent(11.5, 12.5, 8.25, 16.75, 7.25, 6.5))

@icon("DESKTOP")
def _(i):
    i.rect(3, 4.25, 18, 12.25, 2.4)
    i.line((12, 16.75), (12, 19.75))
    i.line((8.5, 19.75), (15.5, 19.75))

@icon("PALETTE")
def _(i):
    blob = [(12, 3.5), (18.3, 5.7), (20.6, 11.2), (18.6, 14.9), (15.4, 14.6), (13.6, 16.3), (14.2, 19.1), (12, 20.5), (5.9, 18.3), (3.5, 12.0), (5.7, 5.8)]
    i.stroke(spline(blob, closed=True))
    i.dot(7.9, 10.4, 1.3)
    i.dot(11.4, 7.6, 1.3)
    i.dot(15.4, 8.6, 1.3)
    i.dot(8.1, 14.7, 1.3)

@icon("BELL_RINGING")
def _(i):
    body = [(4.75, 17), (6.5, 14.4), (6.75, 10.0), (8.9, 6.0), (12, 4.75), (15.1, 6.0), (17.25, 10.0), (17.5, 14.4), (19.25, 17)]
    i.stroke(spline(body))
    i.line((4.75, 17), (19.25, 17))
    i.arc(12, 17.5, 2.6, 30, 150)
    i.arc(12, 11, 10.25, 205, 232)
    i.arc(12, 11, 10.25, 308, 335)

@icon("ARROWS_LEFT_RIGHT")
def _(i):
    i.line((4, 8.25), (20, 8.25))
    i.line((16.5, 4.75), (20, 8.25), (16.5, 11.75))
    i.line((20, 15.75), (4, 15.75))
    i.line((7.5, 12.25), (4, 15.75), (7.5, 19.25))

@icon("CHART_BAR")
def _(i):
    i.rect(4.25, 12, 4, 8, 1.3)
    i.rect(10, 7, 4, 13, 1.3)
    i.rect(15.75, 3.75, 4, 16.25, 1.3)

@icon("ARROWS_DOWN_UP")
def _(i):
    i.line((8, 4.25), (8, 19.75))
    i.line((4.75, 16.5), (8, 19.75), (11.25, 16.5))
    i.line((16, 19.75), (16, 4.25))
    i.line((12.75, 7.5), (16, 4.25), (19.25, 7.5))

@icon("ARROW_RIGHT")
def _(i):
    i.line((4, 12), (19.5, 12))
    i.line((13.5, 6), (19.5, 12), (13.5, 18))

@icon("ARROW_LEFT")
def _(i):
    i.line((20, 12), (4.5, 12))
    i.line((10.5, 6), (4.5, 12), (10.5, 18))

@icon("CARET_LEFT")
def _(i):
    i.line((15, 5), (8, 12), (15, 19))

@icon("CARET_RIGHT")
def _(i):
    i.line((9, 5), (16, 12), (9, 19))

@icon("CARET_DOWN")
def _(i):
    i.line((5, 9), (12, 16), (19, 9))

@icon("CARET_UP")
def _(i):
    i.line((5, 15), (12, 8), (19, 15))

@icon("SKIP_FORWARD")
def _(i):
    i.path([(5.5, 5), (16, 12), (5.5, 19)], rc=1.8, closed=True)
    i.line((19.25, 5), (19.25, 19))

@icon("SKIP_BACK")
def _(i):
    i.path(mirror([(5.5, 5), (16, 12), (5.5, 19)]), rc=1.8, closed=True)
    i.line((4.75, 5), (4.75, 19))

@icon("SLIDERS_HORIZONTAL")
def _(i):
    i.line((3.75, 7.5), (6.75, 7.5))
    i.ring(9.25, 7.5, 2.4)
    i.line((11.75, 7.5), (20.25, 7.5))
    i.line((3.75, 16.5), (12.25, 16.5))
    i.ring(14.75, 16.5, 2.4)
    i.line((17.25, 16.5), (20.25, 16.5))

def eye(i):
    for cy, a0, a1 in ((15.2, 205, 335), (8.8, 25, 155)):
        r = 9.6
        i.arc(12, cy, r, a0, a1)
    i.ring(12, 12, 2.9)

@icon("EYE")
def _(i):
    eye(i)

@icon("EYE_SLASH")
def _(i):
    eye(i)
    i.line((4.25, 4.25), (19.75, 19.75))

@icon("ARROWS_OUT")
def _(i):
    i.path([(4, 9.25), (4, 4), (9.25, 4)], rc=1.4)
    i.path([(14.75, 4), (20, 4), (20, 9.25)], rc=1.4)
    i.path([(20, 14.75), (20, 20), (14.75, 20)], rc=1.4)
    i.path([(9.25, 20), (4, 20), (4, 14.75)], rc=1.4)

@icon("ARROWS_IN")
def _(i):
    i.path([(4, 9), (9, 9), (9, 4)], rc=1.4)
    i.path([(15, 4), (15, 9), (20, 9)], rc=1.4)
    i.path([(20, 15), (15, 15), (15, 20)], rc=1.4)
    i.path([(9, 20), (9, 15), (4, 15)], rc=1.4)

@icon("LIST")
def _(i):
    for y in (6.5, 12, 17.5):
        i.dot(5, y, 1.35)
        i.line((9, y), (20, y))

@icon("BOOK_OPEN")
def _(i):
    left = spline([(12, 7.0), (8.2, 5.3), (3.25, 5.25)]) + [(3.25, 17.6)] + spline([(3.25, 17.6), (8.2, 17.7), (12, 19.25)])
    i.stroke(left)
    i.stroke(mirror(left))
    i.line((12, 7.0), (12, 19.25))

@icon("FILE")
def _(i):
    i.rect(5.75, 3.5, 12.5, 17, 2.4)

@icon("SEAL_CHECK")
def _(i):
    pts = []
    for k in range(144):
        t = 2 * math.pi * k / 144
        r = 8.55 + 0.75 * math.cos(12 * t)
        pts.append((12 + r * math.cos(t), 12 + r * math.sin(t)))
    i.stroke(pts + [pts[0]])
    i.line((8.4, 12.3), (10.9, 14.7), (15.6, 9.6))

@icon("LINK")
def _(i):
    left = rounded([(10.25, 8.25), (2.75, 8.25), (2.75, 15.75), (10.25, 15.75)], 3.75, False)
    right = rounded([(13.75, 8.25), (21.25, 8.25), (21.25, 15.75), (13.75, 15.75)], 3.75, False)
    i.stroke(rotate(left, 12, 12, -45))
    i.stroke(rotate(right, 12, 12, -45))
    i.stroke(rotate([(8.25, 12), (15.75, 12)], 12, 12, -45))

@icon("GLOBE")
def _(i):
    i.ring(12, 12, 8.75)
    ell = [(12 + 3.9 * math.cos(t), 12 + 8.75 * math.sin(t)) for t in [2 * math.pi * k / 80 for k in range(81)]]
    i.stroke(ell)
    i.line((3.5, 12), (20.5, 12))

@icon("USER")
def _(i):
    i.ring(12, 8.5, 4.25)
    i.stroke(spline([(4.25, 20.25), (6.9, 16.3), (12, 14.8), (17.1, 16.3), (19.75, 20.25)]))

@icon("ARROW_SQUARE_OUT")
def _(i):
    i.path([(10.5, 5), (5, 5), (5, 19), (19, 19), (19, 13.5)], rc=2.2)
    i.line((14, 5), (19, 5), (19, 10))
    i.line((19, 5), (11.5, 12.5))

@icon("EXPORT")
def _(i):
    i.line((12, 14), (12, 3.5))
    i.line((8.5, 7), (12, 3.5), (15.5, 7))
    i.path([(8.5, 10), (5, 10), (5, 20.25), (19, 20.25), (19, 10), (15.5, 10)], rc=2.2)

@icon("COPY")
def _(i):
    i.rect(8.5, 8.5, 11.5, 11.5, 2.2)
    i.path([(15.5, 8), (15.5, 4), (4, 4), (4, 15.5), (8, 15.5)], rc=2.2)

@icon("SQUARES_FOUR")
def _(i):
    for x, y in ((4, 4), (13, 4), (4, 13), (13, 13)):
        i.rect(x, y, 7, 7, 2.0)

@icon("PENCIL")
def _(i):
    i.path([(15.5, 4.5), (19.5, 8.5), (8.5, 19.5), (4.5, 19.5), (4.5, 15.5)], rc=1.2, closed=True)
    i.line((13, 7), (17, 11))

@icon("SIGN_OUT")
def _(i):
    i.path([(10, 4), (5, 4), (5, 20), (10, 20)], rc=2.0)
    i.line((10, 12), (20, 12))
    i.line((16.5, 8.5), (20, 12), (16.5, 15.5))

@icon("TRANSLATE")
def _(i):
    i.line((3.5, 6), (12.5, 6))
    i.line((8, 3.75), (8, 6))
    i.stroke(spline([(11, 6), (9.8, 10.4), (5.6, 14.1)]))
    i.stroke(spline([(6.4, 8.5), (8.6, 12.4), (11.6, 14)]))
    i.line((12.5, 20.25), (16.25, 11.25), (20, 20.25))
    i.line((13.75, 17.25), (18.75, 17.25))


def build(out_path):
    names = list(ICONS)
    order = [".notdef"] + names
    fb = FontBuilder(UPM, isTTF=True)
    fb.setupGlyphOrder(order)
    cmap = {BASE + k: n for k, n in enumerate(names)}
    fb.setupCharacterMap(cmap)
    glyphs = {".notdef": TTGlyphPen(None).glyph()}
    for n in names:
        glyphs[n] = glyph(ICONS[n])
    fb.setupGlyf(glyphs)
    # the left side bearing has to match each outline, renderers that hint place glyphs by it
    glyf = fb.font["glyf"]
    fb.setupHorizontalMetrics({n: (UPM, getattr(glyf[n], "xMin", 0)) for n in order})
    fb.setupHorizontalHeader(ascent=ASC, descent=DESC)
    fb.setupNameTable({"familyName": "Yomikaze Icons", "styleName": "Regular"})
    fb.setupOS2(sTypoAscender=ASC, sTypoDescender=DESC, usWinAscent=ASC, usWinDescent=-DESC)
    fb.setupPost()
    fb.save(out_path)
    return names


RUST_HEADER = """// yomikaze's own icons, drawn in tools/make_icons.py and stored in assets/yomikaze-icons.ttf.
// anything not drawn here still comes from phosphor.
#![allow(dead_code)]

pub use egui_phosphor::regular::*;

"""


# python tools/make_icons.py assets/yomikaze-icons.ttf src/icons.rs
if __name__ == "__main__":
    names = build(sys.argv[1])
    with open(sys.argv[2], "w", encoding="utf-8", newline="\n") as f:
        f.write(RUST_HEADER)
        for k, n in enumerate(names):
            f.write(f'pub const {n}: &str = "\\u{{{BASE + k:X}}}";\n')
    print(len(names), "icons")
