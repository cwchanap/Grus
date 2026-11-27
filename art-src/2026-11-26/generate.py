#!/usr/bin/env python3
"""HPA-1210 raw art: painted unit poses and intro story panels, generated with Codex.

Every image is one non-interactive Codex run using its built-in image generation:

    codex exec --model gpt-6-luna -c model_reasoning_effort=xhigh -s workspace-write \
        -C <repo> -i <reference.png>... -   (prompt on stdin)

Run from the repository root:

    python3 art-src/2026-11-26/generate.py base   [kind ...]   # units/<kind>.png from the HUD portrait
    python3 art-src/2026-11-26/generate.py action [kind ...]   # units/<kind>_action.png from base + portrait
    python3 art-src/2026-11-26/generate.py panels [n ...]      # intro/panel_<n>.png from the Start key art

Raw outputs land next to this file; `process.py` turns them into the runtime copies.
"""

import os
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
REL = HERE.relative_to(REPO)
CODEX = ["codex", "exec", "--model", "gpt-6-luna", "-c", "model_reasoning_effort=xhigh", "-s", "workspace-write"]
LOGS = Path(tempfile.gettempdir()) / "hpa1210-codex-logs"

PORTRAIT = "godot/assets/ui/icons/units/{}.png"
BUILDING = "godot/assets/structures/painted/barracks.png"
KEY_ART = "godot/assets/ui/frostline/screens/start_key_art.png"

STYLE = (
    "Painted game art in the cohesive 'Frostline' winter-survival style of the references: cold blue-grey "
    "steel and packed snow, warm orange accents, cyan light for tech and alien eyes. Soft painterly shading, "
    "lit from the upper left, clean readable silhouette with a slight dark outline. No text, letters or numbers."
)

UNIT_VIEW = (
    "View: high three-quarter view like the attached building sprite: an orthographic camera tilted about "
    "54 degrees down, so we look down a little onto the head and shoulders (or onto the back of a creature). "
    "The unit faces RIGHT: body turned toward the right edge of the frame, three-quarter profile. The whole "
    "body is visible, nothing cropped. Feet at the bottom centre of the canvas, unit centred horizontally, "
    "filling most of the frame with a small margin. The unit is only about 20 px tall in play, so keep a "
    "strong, simple, chunky silhouette with clear shapes and good contrast against snow. "
    "No shadow, no ground, no snow patch, no scenery: the unit alone on a fully transparent background "
    "(RGBA PNG; a contact shadow is added later). If transparency is impossible, use a flat solid #FF00FF "
    "background with nothing else on it, and say so. Square image."
)

HUMAN = "Colours: cool blue-grey cold-weather gear with warm orange accents, exactly as in the portrait."
ALIEN = (
    "Colours: dark slate-grey frosted chitin with cool highlights so it reads as hostile against snow; glowing "
    "{glow} accents. Keep the portrait's body design (shapes, plates, legs, claws, horns); the portrait's "
    "orange stripe markings stay small and subdued."
)

UNITS = {
    "worker": (
        "the Worker: fur-hooded blue-grey parka, goggles, orange scarf and orange arm stripes, big backpack with "
        "a bedroll and coiled rope, cyan canister and pouch on the belt, a steel pick. " + HUMAN,
        "standing ready, relaxed, pick held at the side",
        "swinging the pick down hard in front of him, mid-swing, body leaning into the blow",
    ),
    "guard": (
        "the Guard: fur-hooded armoured blue-grey parka, orange scarf, backpack with bedroll, cyan lantern on the "
        "belt, a tall spear with a small orange pennant and a large steel shield with a snowflake emblem and "
        "orange stripes. " + HUMAN,
        "standing guard, shield held up in front on the right side, spear upright",
        "thrusting the spear forward to the right past the edge of the raised shield, braced behind the shield",
    ),
    "rifleman": (
        "the Rifleman: fur-hooded blue-grey parka with goggles on the hood, orange face scarf and orange arm "
        "stripes, backpack with bedroll, cyan canister on the belt, a long rifle with a wooden stock. " + HUMAN,
        "standing alert, rifle held across the body at the ready",
        "firing the rifle to the right from the shoulder, aiming, a small bright orange muzzle flash at the barrel tip",
    ),
    "swarmer": (
        "the Swarmer: a small, fast, spider-like alien with spiky segmented armour plates, thin sharp legs, "
        "mandibles and two glowing eyes. " + ALIEN.format(glow="cyan"),
        "crouched on its legs, alert, ready to pounce",
        "lunging forward to the right to bite, front legs raised, mandibles wide open",
    ),
    "spitter": (
        "the Spitter: a crawling many-legged alien with a big glowing green venom sac on its back, armoured head "
        "with fangs and glowing eyes. " + ALIEN.format(glow="cyan eyes and a green venom-sac"),
        "standing on its legs, mouth closed, venom sac glowing softly; no spit, no acid, no droplets",
        "rearing up and spitting a short arc of glowing green acid to the right, venom sac glowing bright green",
    ),
    "brute": (
        "the Brute: a hulking, hunched, gorilla-like alien with heavy spiked armour plates, violet-grey "
        "underbelly and arms, huge curved bone claws and glowing eyes. " + ALIEN.format(glow="cyan and violet"),
        "standing hunched, knuckles and claws near the ground, turned to face right in a three-quarter side view: head and body point toward the right edge, we see more of its left side than its front, not facing the viewer",
        "an overhead smash: both clawed arms raised high above its head, about to slam down",
    ),
    "behemoth": (
        "the Behemoth: the biggest, heaviest alien, upright on massive armoured legs, with two huge horns curving "
        "up and outward from its head like a crescent (as in the portrait), heavy armoured arms, spikes, claws and "
        "glowing cyan eyes. Colours exactly like the portrait: frosted slate-grey chitin plates with snow and frost "
        "caked on them and small subdued orange stripe markings, faint violet glow in the joints; not black. Also "
        "attached: the approved Brute sprite art-src/2026-11-26/units/brute.png, the same alien family: match its "
        "rendering, frost and colours.",
        "standing tall and heavy, legs planted wide, turned to face right in a three-quarter side view: head, horns and body point toward the right edge, we see more of its left side than its front, not facing the viewer",
        "roaring with its maw open and stomping one huge foot down, arms spread wide",
    ),
}

# Extra base-pose reference: an approved sibling sprite to keep an alien on-model with its family.
FAMILY_REFS = {"behemoth": [str(REL / "units" / "brute.png")]}

PANEL_RULES = (
    "Wide 16:9 landscape illustration with a full painted background, fully opaque. Match the attached Start "
    "key art exactly in painted style, palette, lighting, snow and level of detail: night or storm-dark blues, "
    "drifting snow, warm orange lights, cyan alien/research glow. No text, no letters, no logos, no letterbox "
    "or borders. Composition rules: keep the lower third calm and fairly dark (soft snow, fog or plain ground, "
    "no key detail there) because a caption band is laid over it; put the key subject in the upper two thirds; "
    "keep a margin of about 8% at every edge free of key detail, because the image slowly pans and zooms."
)

PANELS = {
    1: (
        [KEY_ART],
        "Story panel 1: a dim, pale sun barely glowing behind a heavy grey storm-cloud wall over a frozen Earth: "
        "an endless ice-covered landscape and frozen sea under the clouds, a buried city's tiny frozen towers far "
        "below. High in the clouds the first dark alien spires pierce down through the storm, faint cyan and "
        "violet glows on them. Mood: cold, ominous, quiet, the moment the winter began.",
    ),
    2: (
        [KEY_ART, PORTRAIT.format("swarmer"), PORTRAIT.format("brute")],
        "Story panel 2: night. The skyline of an ordinary frozen city, a different city from the key art: tall "
        "snow-covered skyscrapers and apartment towers standing against the stormy night sky, seen from a low, "
        "distant viewpoint across a frozen plain. Do NOT show a glass dome, the Research Center, or the walled "
        "city of the key art. A swarm of dark chitin aliens (like the attached Swarmer and Brute) pours over the "
        "skyline like a black flood, swarming over the tops of the towers and streaming down their sides, "
        "hundreds of small glowing cyan and violet eyes, blizzard. A few warm orange windows still lit in the "
        "towers. Terrifying but readable.",
    ),
    3: (
        [KEY_ART],
        "Story panel 3: a dead city skyline at night in a blizzard, seen from far away: frozen skyscrapers, most "
        "windows dark, the last few clusters of warm orange lights going out one by one across the skyline (lit "
        "on one side, dark on the other), a radio mast with a dying faint red beacon. Cold, silent and lonely.",
    ),
    5: (
        [KEY_ART, "godot/assets/structures/painted/research_center.png",
         PORTRAIT.format("guard"), PORTRAIT.format("rifleman")],
        "Story panel 5: the Research Center of the last city at night in a blizzard: the large fortified lab "
        "with a glass geodesic dome glowing bright cyan from inside (same design as the attached Research "
        "Center), seen from just outside the city wall. The snow-covered wall with turrets and orange banners "
        "crosses the middle of the image, small defenders in blue parkas (like the attached Guard and Rifleman) "
        "standing on it with rifles and spears, warm orange torches; the glowing dome rises behind them. Tense "
        "but hopeful.",
    ),
}


def save_rule(out: Path) -> str:
    return (
        f"Save the generated image, unmodified, as {out} (copy the original file the image tool produced; do "
        "not re-encode, resize or edit it). Then reply with the original generated file path, its pixel size "
        "and mode, and whether the background is transparent."
    )


def unit_job(kind: str, action: bool):
    design, base_pose, action_pose = UNITS[kind]
    portrait = PORTRAIT.format(kind)
    if not action:
        out = REL / "units" / f"{kind}.png"
        refs = [portrait, BUILDING] + FAMILY_REFS.get(kind, [])
        lead = (
            f"Attached references (also on disk): the HUD portrait {portrait} (the unit design to match exactly) "
            f"and the painted building sprite {BUILDING} (camera angle and rendering style).\n\n"
            f"Image: a single full-body unit sprite for a top-down real-time strategy game of {design}\n\n"
            f"Pose (base pose): {base_pose}."
        )
    else:
        base = REL / "units" / f"{kind}.png"
        out = REL / "units" / f"{kind}_action.png"
        refs = [str(base), portrait, BUILDING]
        lead = (
            f"Attached references (also on disk): {base} is the approved BASE pose sprite of this unit; "
            f"{portrait} is its HUD portrait; {BUILDING} shows the camera angle.\n\n"
            f"Image: the ACTION pose of the same unit, {design}\n\n"
            f"Pose (action pose): {action_pose}.\n\n"
            "It must read as exactly the same character as the base pose sprite: same design, colours, gear, "
            "proportions and rendering. Same scale: the body is the same size as in the base pose image, and "
            "the feet stand at the same spot at the bottom centre of the canvas, so switching between the two "
            "images in game does not jump."
        )
    prompt = (
        "Use your built-in image generation tool to create ONE image, then save it. Do not write code to draw "
        "it; it must come from image generation.\n\n"
        f"{lead}\n\n{UNIT_VIEW}\n\nStyle: {STYLE}\n\n{save_rule(out)}"
    )
    return out, refs, prompt


def panel_job(n: int):
    refs, subject = PANELS[n]
    out = REL / "intro" / f"panel_{n}.png"
    prompt = (
        "Use your built-in image generation tool to create ONE image, then save it. Do not write code to draw "
        "it; it must come from image generation.\n\n"
        f"Attached references (also on disk): {', '.join(refs)}. The first one, the Start key art, is the style, "
        "palette and lighting reference; this image is another panel of the same intro story.\n\n"
        f"{subject}\n\n{PANEL_RULES}\n\n{save_rule(out)}"
    )
    return out, refs, prompt


def run(job) -> str:
    out, refs, prompt = job
    LOGS.mkdir(parents=True, exist_ok=True)
    log = LOGS / f"{out.stem}.log"
    cmd = CODEX + ["-C", str(REPO)]
    for r in refs:
        cmd += ["-i", r]
    cmd += ["-o", str(LOGS / f"{out.stem}.last.txt"), "-"]
    start = time.time()
    with open(log, "w") as fh:
        rc = subprocess.run(cmd, input=prompt, text=True, cwd=REPO, stdout=fh, stderr=subprocess.STDOUT).returncode
    path = REPO / out
    ok = rc == 0 and path.exists() and path.stat().st_mtime >= start
    return f"{'OK  ' if ok else 'FAIL'} {out} rc={rc} {time.time() - start:.0f}s log={log}"


def main(argv):
    if not argv or argv[0] not in ("base", "action", "panels"):
        sys.exit(__doc__)
    mode, names = argv[0], argv[1:]
    if mode == "panels":
        jobs = [panel_job(int(n)) for n in (names or PANELS)]
    else:
        jobs = [unit_job(k, mode == "action") for k in (names or UNITS)]
    for _, _, p in jobs[:1]:
        print(p, "\n---", flush=True)
    with ThreadPoolExecutor(max_workers=int(os.environ.get("JOBS", "7"))) as pool:
        for line in pool.map(run, jobs):
            print(line, flush=True)


if __name__ == "__main__":
    main(sys.argv[1:])
