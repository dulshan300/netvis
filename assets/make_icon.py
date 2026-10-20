# Draws the NetVis app icon: python3 assets/make_icon.py assets  (needs Pillow)
import sys
from PIL import Image, ImageDraw
N = 1024
img = Image.new("RGBA", (N, N), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
d.rounded_rectangle((32, 32, N - 32, N - 32), radius=210, fill=(37, 99, 235, 255))
W = (255, 255, 255, 255)
top, left, mid, right = (512, 280), (270, 730), (512, 730), (754, 730)
bus = 505
lw = 52
d.line([top, (512, bus)], fill=W, width=lw)
d.line([(left[0], bus), (right[0], bus)], fill=W, width=lw)
for x in (left[0], mid[0], right[0]):
    d.line([(x, bus), (x, 730)], fill=W, width=lw)
def node(c, r, fill):
    d.rounded_rectangle((c[0] - r, c[1] - r * 0.78, c[0] + r, c[1] + r * 0.78), radius=r * 0.3, fill=fill)
node(top, 130, W)
d.rounded_rectangle((top[0] - 80, top[1] - 12, top[0] + 80, top[1] + 12), radius=10, fill=(37, 99, 235, 255))
for c in (left, mid, right):
    node(c, 92, W)
out = sys.argv[1]
sizes = [16, 24, 32, 48, 64, 128, 256]
img.resize((256, 256), Image.LANCZOS).save(f"{out}/netvis.png")
img.resize((256, 256), Image.LANCZOS).save(f"{out}/netvis.ico", sizes=[(s, s) for s in sizes])
