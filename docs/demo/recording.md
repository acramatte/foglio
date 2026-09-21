# Re-recording the README demo video

How `docs/demo/demo.mp4` / `demo.gif` are produced: a script drives a real
`foglio-desktop` window through a scripted tour (WebDriver + visible click
ripples + human-speed typing) while a screen recorder captures it, then the
take is trimmed and cropped with ffmpeg.

> **Machine-specific by design.** The window position/size depends on the
> local window manager's tiling, the recorder talks to the local GNOME
> portal, and WebKitGTK quirks vary by build. The steps below were done on
> the author's machine (GNOME Wayland, ultrawide 3440×1440, Ubuntu) and will
> need recalibration elsewhere — each step notes what to re-measure.

## Prerequisites

- Desktop binary built with the frontend embedded (a plain `cargo build`
  expects a dev server and shows "Could not connect to localhost"):

  ```sh
  cd apps/desktop && npm run tauri -- build --debug --no-bundle
  ```

- `tauri-driver` + `WebKitWebDriver` (same requirements as
  `tests/acceptance/phase4.py`).
- A screen recorder that works on GNOME Wayland. This recording used
  [GPU Screen Recorder](https://flathub.org/apps/com.dec05eba.gpu_screen_recorder)
  via its portal capture:

  ```sh
  flatpak install flathub com.dec05eba.gpu_screen_recorder
  ```

## 1. Record

Run the recorder **first** and approve GNOME's share dialog (choose the
whole monitor — window capture exists but stalls after ~25s on this setup),
then run the choreography:

```sh
# terminal 1: recorder; SIGINT (Ctrl-C) it to finalize the file — SIGTERM truncates
flatpak run --command=gpu-screen-recorder com.dec05eba.gpu_screen_recorder \
  -w portal -f 30 -c mkv -q medium -k h264 -o /tmp/demo.mkv

# terminal 2: the scripted tour (starts the app itself, isolated demo library)
python3 docs/demo/record-demo.py
```

`record-demo.py` is self-contained: it builds a disposable demo library
(fictional notes under an isolated `XDG_CONFIG_HOME`/`XDG_CACHE_HOME`, so
real notes can never appear), starts the app via tauri-driver, and performs
the tour — light theme → open note → typed search ("car*" notes narrow to
one) → new note with typed title → fast-typed Markdown body in Source →
Preview → dark theme finale. See the `SCENE:` log lines for the timeline.

## 2. Find the take boundaries

The demo window may tile to a different spot each run, so re-measure instead
of reusing coordinates. The light-theme window is the only large bright
rectangle on screen — its bounding box gives the crop directly:

```python
import numpy as np, subprocess
from PIL import Image
subprocess.run(["ffmpeg","-y","-v","error","-ss","50","-i","/tmp/demo.mkv",
                "-frames:v","1","/tmp/frame.png"], check=True)
im = np.asarray(Image.open("/tmp/frame.png").convert("RGB"), dtype=np.uint8)
bright = im.mean(axis=2) > 190
colc, rowc = bright.sum(axis=0), bright.sum(axis=1)
cols = np.where(colc > 400)[0]; rows = np.where(rowc > 400)[0]
print("window rect:", cols.min(), rows.min(), cols.max()-cols.min()+1, rows.max()-rows.min()+1)
```

(For dark-theme frames, the "New note" button's olive color
`rgb(177,188,115)` is a reliable landmark: find it and offset by the button's
position inside the window.) The demo starts when the window appears and ends
at the last `SCENE:` marker + ~5s hold; a brightness scan at 1s steps finds
both edges.

## 3. Trim, crop, encode

The crop below matches the measured rect with a few pixels of margin; the
first seconds before the first scene are cut:

```sh
SS=34.8; TO=69.2           # take boundaries from step 2
X=1121; Y=384; W=1197; H=750  # window rect + margin from step 2

ffmpeg -y -v error -ss $SS -to $TO -i /tmp/demo.mkv \
  -vf "crop=$W:$H:$X:$Y,scale=1152:-2,fps=30" \
  -c:v libx264 -crf 23 -preset slow -pix_fmt yuv420p -an demo.mp4

# GIF: smaller proxy first, then a perceptual palette
ffmpeg -y -v error -ss $SS -to $TO -i /tmp/demo.mkv \
  -vf "crop=$W:$H:$X:$Y,scale=1000:-2,fps=12" -an /tmp/pal_in.mp4
ffmpeg -y -v error -i /tmp/pal_in.mp4 -vf "palettegen=stats_mode=diff" /tmp/pal.png
ffmpeg -y -v error -i /tmp/pal_in.mp4 -i /tmp/pal.png \
  -lavfi "paletteuse=dither=bayer:bayer_scale=4" demo.gif
```

## 4. Embed

GitHub renders an MP4 in the README only from a `user-attachments` URL:
drag `demo.mp4` into any issue/PR comment box, copy the generated
`https://github.com/user-attachments/assets/<uuid>` URL, and paste it as a
bare line in the README (the attachment survives deleting the issue). The
GIF can be committed alongside and referenced with an `<img loading="lazy">`
tag as a fallback.

## Quirks hit along the way (WebKitWebDriver / GNOME)

- A leftover `foglio-desktop` process makes relaunches exit via the
  single-instance plugin → WebDriver sees "no such frame" / "unload".
  Kill strays before a take.
- `element/value` with an empty `text` is rejected ("Missing text
  parameter") — clear the field instead, and fire the `input` event
  manually or the app never sees the cleared value.
- The source `<textarea>` is not WebDriver-focusable right after creation;
  `type_slow` types through JS `setRangeText` + dispatched `input` events,
  which autosave picks up normally.
- `document.documentElement.requestFullscreen()` is rejected without
  transient activation, so the recording keeps whatever is behind/around the
  window — hence the crop.
- `SIGTERM` on the recorder truncates the MKV tail; always `SIGINT`/Ctrl-C
  (kill the inner `gpu-screen-recorder` process, not the flatpak wrapper).

## Privacy

`record-demo.py` runs the app against a throwaway library of fictional notes
in an isolated sandbox — real notes are never read or shown. Still, scrub a
few frames of the final video before publishing; the recorder captures the
monitor, so anything raised over the app during the take would appear.
