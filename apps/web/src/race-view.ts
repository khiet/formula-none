import {
  AmbientLight,
  Box3,
  BoxGeometry,
  BufferGeometry,
  Color,
  DirectionalLight,
  DoubleSide,
  Group,
  Mesh,
  MeshBasicMaterial,
  type Object3D,
  PerspectiveCamera,
  PointLight,
  Quaternion,
  RingGeometry,
  SphereGeometry,
  Scene as ThreeScene,
  Vector3,
} from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { CSS2DObject, CSS2DRenderer } from "three/addons/renderers/CSS2DRenderer.js";
import { WebGPURenderer } from "three/webgpu";
import type { Coin, ItemBox, KartReadout, KartSize, Projectile, Tuning, Wheel } from "./bridge";
import type { CameraPose } from "./chase-camera";
import { coinMeshes, coinSpray } from "./coin-meshes";
import { createDustEmitter } from "./dust";
import { createExhaustFlames } from "./exhaust-flame";
import { hazardZones } from "./hazard-zone";
import { HEAD_LEVEL, type HeadSway, swayHead } from "./head-sway";
import { type DroneFlight, flyDrones } from "./homing-drones";
import {
  type Beacon,
  type HomingDrone,
  hazardConeMesh,
  homingDroneMesh,
  itemBoxMeshes,
  type Marshal,
  marshalBeacon,
  marshalMesh,
  runawayWheelBounce,
  runawayWheelFlames,
  runawayWheelMesh,
} from "./item-meshes";
import { glint } from "./item-sprites";
import { groundDistance, type Vec3 } from "./kart-frame";
import type { KartLabel } from "./kart-labels";
import { dressKart, fitKart, type KartFit, shineKart } from "./kart-model";
import { createKartShadows, type SurfaceProbe } from "./kart-shadows";
import { hexCss, livery } from "./livery";
import type { Point } from "./low-poly-svg";
import { HAZARD_ORANGE, plate, tonesOf, writePalette } from "./palette";
import { createParticles } from "./particles";
import { type PitBoxTuning, pitBoxMeshes } from "./pit-box-meshes";
import type { PresentationBlend } from "./presentation-zones";
import { BLAST_S, type Blast, type RaceReadout } from "./race-run";
import { sceneryMotion } from "./scenery-motion";
import { createSparkEmitter, createSparkParticles } from "./sparks";
import { spinOutAngle } from "./spin";
import { createStarSparkles, starShine } from "./star-shine";
import { DRIVER_LOOK_RAD, driverLook, kartLift } from "./stop-beats";
import {
  additiveMaterial,
  exhaustFlameMesh,
  particleMesh,
  shadowDiscMesh,
  tyreMarkMesh,
} from "./tell-meshes";
import type { PitBox } from "./track";
import { trackProbe } from "./track-probe";
import { createTyreMarks } from "./tyre-marks";
import { kartWheels } from "./wheel-spin";
import { createWheelTellEmitter, wheelBack } from "./wheel-tells";
import { createWindLines, createWindParticles, windFade } from "./wind-lines";

/** Far enough to see across Bell Deer, about 430 m by 270 m, from any point on it. */
const CAMERA_FAR_M = 1000;

/** The course's own light, which a presentation zone scales (ADR 0098). */
const SUN_INTENSITY = 2;
const AMBIENT_INTENSITY = 0.6;
const WHITE = new Color(0xffffff);
/** The tunnel lamps' warm, the track's `lamp` material (assets/tracks/build.py). */
const LAMP_COLOUR = new Color().setRGB(1.0, 0.851, 0.541);
/** The lamps' light hangs at their height over the own kart and reaches the karts round it. */
const LAMP_HEIGHT_M = 4.4;
const LAMP_REACH_M = 14;
/** Under a lamp: about as bright on the kart's top as the open sky's ambient. */
const LAMP_INTENSITY = 20;
/** How far the lamps warm the ambient at their glow's peak. */
const LAMP_AMBIENT_TINT = 0.35;
/**
 * Puffs live at once across the grid: eight karts' worth through their
 * life and a runaway wheel's smoke trail, with room over. A pool full
 * earlier drops its oldest.
 */
const DUST_CAPACITY = 320;
/** Dust hangs, slows and spreads. */
const DUST_DRAG_PER_S = 2;
const DUST_GROWTH_PER_S = 2;
const DUST_OPACITY = 0.55;
/** A puff shrinks away over the last share of its life. */
const DUST_FADE_SHARE = 0.4;
/** A red spark's halo is this many times as wide as the spark, and this bright at birth. */
const SPARK_HALO_SCALE = 5;
const SPARK_HALO_BRIGHTNESS = 0.45;
/** How high the projectile (ADR 0007) flies over where the sim has it. */
const PROJECTILE_HEIGHT_M = 0.6;
/**
 * The star's halo (ADR 0075): an additive sphere round the kart this big,
 * this bright at the full pull, so the shine carries at chase-camera
 * distance. Its sparkles' pool holds this many per kart; the hit's burst
 * is thrown where a kart spins out within this much of a star kart.
 */
const STAR_HALO_RADIUS_M = 1.6;
const STAR_HALO_OPACITY = 0.1;
const STAR_SPARKLE_CAPACITY_PER_KART = 48;
const STAR_SPARKLE_DRAG_PER_S = 1.5;
const STAR_SPARKLE_COOL_SHARE = 0.5;
const STAR_HIT_REACH_M = 4;
/**
 * The Air Horn's blast (ADR 0076): a flat orange ring that grows from the
 * firing kart to the blast's reach in this long, this wide, lifted this
 * far off the road and this opaque, fading out over the blast's time, so
 * the reach can be learned by eye.
 */
const HORN_RING_GROW_S = 0.25;
const HORN_RING_WIDTH_M = 0.6;
const HORN_RING_LIFT_M = 0.05;
const HORN_RING_OPACITY = 0.85;
/**
 * On a marshal post the marshal stands this far off its middle, to the
 * left as seen from the track, and the beacon this far toward the track
 * and to the right: its track-side corner.
 */
const MARSHAL_OFF_MIDDLE_M = -0.05;
const BEACON_CORNER_M = 0.55;
/** The runaway wheel's flame tongues flicker this fast, each out of step, through this share of their length. */
const WHEEL_FLAME_FLICKER_RAD_PER_S = 30;
const WHEEL_FLAME_FLICKER_STEP_RAD = 2.1;
const WHEEL_FLAME_FLICKER_SHARE = 0.3;
/**
 * Through the warning the wheel waits on the wall top, this far past the
 * wall's face it rolls in from so it stands on the wall and not its edge,
 * rocking about its axle this fast through this half-swing. The wall's top
 * is the highest surface no more than this over the wall's foot.
 */
const WHEEL_WAITS_PAST_FACE_M = 0.25;
const WHEEL_WAIT_ROCK_RAD_PER_S = 9;
const WHEEL_WAIT_ROCK_RAD = 0.15;
const WALL_PROBE_FROM_M = 3;

export interface RaceView {
  /**
   * Move every kart mesh to the transform `race` reports for it, spinning
   * one that is spun out, run every kart's tells `frameSeconds` on from
   * what the readouts say and the phase (the rocket start's sparks show
   * only while it counts down), hang the labels the karts call for and
   * hide every other kart's, move the camera to `camera`, show each item
   * box that is there to pick up, a cone at each hazard and a drone at
   * each projectile, turned toward the kart it homes in on and tilted
   * into its turn, and at each
   * wheel crossing the rolling wheel with its tells, the marshal posed
   * for it, the beacon lit and the hazard zone flashing while the flag is
   * up, and the wheel waiting on the wall through the warning.
   */
  update(race: RaceReadout, camera: CameraPose, frameSeconds: number): void;
  /**
   * Light the world for the presentation zone the own kart is in (ADR
   * 0098): the sky's light scaled by `blend.light` and the tunnel's warm
   * lamps over the own kart by `blend.lampGlow`, as of the last `update`.
   */
  presentation(blend: PresentationBlend): void;
  render(): void;
  /** Take the canvas off the page and free the renderer. */
  dispose(): void;
}

/** Who the View is drawn for. */
export interface ViewerOptions {
  /** The own kart's slot: the wind lines of a boost are drawn round it alone. */
  readonly localKart: number;
  /** The viewer asked for less motion: fewer wind lines. */
  readonly reducedMotion: boolean;
}

/** The glTF files the View draws: the track and the kart every slot is a copy of. */
export interface ViewAssets {
  readonly trackGltfUrl: string;
  readonly kartGltfUrl: string;
}

/**
 * What the View reads of the sim's tuning: the karts on the grid and their
 * box, and every tuned length, reach and life a mechanic's Feedback is drawn to.
 */
export type ViewTuning = PitBoxTuning &
  Pick<
    Tuning,
    | "kartsPerRace"
    | "kartSizeM"
    | "spinOutS"
    | "wheelRadiusM"
    | "hornRadiusM"
    | "slipstreamChargeS"
    | "tyreLifeCliffPercent"
  >;

/**
 * The Race Scene's View: the track's glTF, the kart's glTF once per slot in
 * the slot's livery and a box per item box, drawn by Three.js from the
 * kart readouts the bridge reports and the pose the chase camera computes,
 * with a name floating over each kart the Scene labels, a cone at each
 * hazard, a drone at each projectile and a coin at each of `coins` while
 * it is there to pick up, every box and coin turning on the View's clock,
 * and at each of `pitBoxes` its mark, its light and its crew, lit and posed
 * by its slot's stop (ADR 0082). The
 * track mesh is Presentation
 * only; the sim has its own copy of the geometry from the sidecar. Every
 * mechanic's Feedback is drawn from the readouts and the phase alone:
 * tyre marks under a drift, darker off worn tyres, light smoke off them too, sparks once a tier has charged, drifting or
 * holding for the rocket start, dust on the run-off and at the wheels of a
 * stalled kart, a spun-out kart turned about its axis over
 * `tuning.spinOutS`, a shadow disc on the track under every kart so a
 * hop's height reads against the ground, a flame at the exhaust of every
 * boosting kart, a spray of the coins a kart's spin-out cost it as its
 * count falls and, round the own kart alone, wind lines while it boosts.
 * At each wheel crossing a runaway wheel `tuning.wheelRadiusM` in size rolls
 * where the readout has it, burning, its flames off its back, trailing
 * smoke, kicking up dust where it lands and striking sparks off each wall
 * it rebounds from, the marshal sits with his tea or waves his hazard
 * flags as the wheel has it, and the post's beacon is lit while the flag
 * is up.
 * At each horn blast a flat orange ring grows to `tuning.hornRadiusM`, the blast's
 * reach, and fades.
 * `labelsFor` names the karts to label and where, from the frame's readouts. Picks WebGPU when the
 * browser has it and WebGL otherwise.
 */
export async function createRaceView(
  container: HTMLElement,
  assets: ViewAssets,
  itemBoxes: readonly ItemBox[],
  coins: readonly Coin[],
  pitBoxes: readonly PitBox[],
  tuning: ViewTuning,
  labelsFor: (karts: readonly KartReadout[]) => readonly KartLabel[],
  viewer: ViewerOptions,
): Promise<RaceView> {
  const kartCount = tuning.kartsPerRace;
  const [width, height, length] = tuning.kartSizeM;
  const kartSize: KartSize = { width, height, length };
  const renderer = new WebGPURenderer({ antialias: true });
  await renderer.init();
  renderer.setPixelRatio(window.devicePixelRatio);
  renderer.setSize(container.clientWidth, container.clientHeight);
  container.appendChild(renderer.domElement);
  // The labels are page text laid over the canvas where the camera sees their anchors.
  const labelRenderer = new CSS2DRenderer();
  labelRenderer.setSize(container.clientWidth, container.clientHeight);
  labelRenderer.domElement.dataset.kartLabels = "";
  writePalette(labelRenderer.domElement);
  container.appendChild(labelRenderer.domElement);

  // Three's scene graph, not a glossary Scene: this View belongs to the Race Scene.
  const world = new ThreeScene();
  world.background = new Color(0x87ceeb);
  // The chase camera's pose sets the field of view from the first frame.
  const camera = new PerspectiveCamera(
    60,
    container.clientWidth / container.clientHeight,
    0.1,
    CAMERA_FAR_M,
  );

  const loader = new GLTFLoader();
  const [track, kartModel] = await Promise.all([
    loader.loadAsync(assets.trackGltfUrl),
    loader.loadAsync(assets.kartGltfUrl),
  ]);
  world.add(track.scene);

  const sun = new DirectionalLight(0xffffff, SUN_INTENSITY);
  sun.position.set(20, 40, 10);
  const ambient = new AmbientLight(0xffffff, AMBIENT_INTENSITY);
  // The tunnel's lamps on the own kart: dark in the open sky.
  const lampLight = new PointLight(LAMP_COLOUR, 0, LAMP_REACH_M, 2);
  world.add(sun, ambient, lampLight);
  let ownPosition: readonly [number, number, number] | undefined;

  const box = new Box3().setFromObject(kartModel.scene);
  const fit = fitKart({ min: box.min.toArray(), max: box.max.toArray() }, kartSize);
  const karts = Array.from({ length: kartCount }, (_, slot) => {
    const model = dressKart(kartModel.scene, livery(slot));
    // Measured before the fit scales the model, then drawn at its scale.
    const wheels = kartWheels(model, fit.scale);
    const kart = kartPresentation(model, fit);
    world.add(kart);
    // Missing only from a model without one; the asset test holds kart.glb to carrying it.
    const head = model.getObjectByName("head");
    /** Each kart measures its turn from the readout drawn for it last frame. */
    const sway: { state: HeadSway; last: KartReadout | undefined } = {
      state: HEAD_LEVEL,
      last: undefined,
    };
    // The star's halo rides the kart; hidden off a run.
    const haloMaterial = additiveMaterial();
    const halo = new Mesh(new SphereGeometry(STAR_HALO_RADIUS_M, 12, 8), haloMaterial);
    halo.visible = false;
    kart.add(halo);
    return { model, kart, wheels, head, sway, halo, haloMaterial, shining: false, spunOut: false };
  });

  const tyreMarks = createTyreMarks(kartCount, kartSize, {
    tyreLifeCliffPercent: tuning.tyreLifeCliffPercent,
  });
  const marks = tyreMarkMesh(tyreMarks);
  const sparks = createSparkParticles(kartCount);
  // Each kart's emitter remembers its tier, so a tier's burst is the kart's own.
  const sparkEmitters = karts.map(() => createSparkEmitter(sparks, kartSize));
  // Unlit so the tier's colour reads whatever the sun does; a spark is a
  // streak along its flight and thins as it dies.
  const sparkMesh = particleMesh(sparks, new BoxGeometry(1, 1, 1), new MeshBasicMaterial(), {
    scaleOf: (size, life) => size * life,
    streak: true,
  });
  const sparkHaloMesh = particleMesh(sparks, new SphereGeometry(0.5, 6, 4), additiveMaterial(), {
    scaleOf: (size, life) => size * SPARK_HALO_SCALE * life,
    include: (spark) => spark.glow,
    brightnessOf: (spark) => SPARK_HALO_BRIGHTNESS * spark.life,
  });
  // The star's sparkles: four-point glints that drift back off a star
  // kart, white cooling to the shine, and burst where it lands a hit.
  const starSparkleParticles = createParticles({
    capacity: kartCount * STAR_SPARKLE_CAPACITY_PER_KART,
    drag: STAR_SPARKLE_DRAG_PER_S,
    coolShare: STAR_SPARKLE_COOL_SHARE,
  });
  const starSparkles = createStarSparkles(starSparkleParticles, kartSize);
  const starSparkleMesh = particleMesh(starSparkleParticles, glintGeometry(), additiveMaterial(), {
    scaleOf: (size, life) => size * Math.sin(Math.PI * life),
  });
  const flames = createExhaustFlames(kartCount, kartSize);
  const flameMesh = exhaustFlameMesh(flames);
  const wind = createWindParticles();
  const windLines = createWindLines(wind, kartSize, {
    reducedMotion: viewer.reducedMotion,
    slipstreamChargeS: tuning.slipstreamChargeS,
  });
  /** How brightly the wind lines show this frame, from the own kart's boost or tow left. */
  let windBrightness = 0;
  const windMesh = particleMesh(wind, new BoxGeometry(1, 1, 1), additiveMaterial(), {
    scaleOf: (size) => size,
    streak: true,
    brightnessOf: () => windBrightness,
  });
  const dust = createParticles({
    capacity: DUST_CAPACITY,
    drag: DUST_DRAG_PER_S,
    growth: DUST_GROWTH_PER_S,
  });
  // One each, so a kart's slow trickle of worn smoke is its own and never
  // lands on the next kart to ask.
  const dustEmitters = Array.from({ length: kartCount }, () =>
    createDustEmitter(dust, kartSize, tuning.tyreLifeCliffPercent),
  );
  // The wheel's smoke shares the dust's pool and mesh: puffs that spread and fade.
  const wheelTells = createWheelTellEmitter({ dust, smoke: dust, sparks }, tuning.wheelRadiusM);
  const dustMesh = particleMesh(
    dust,
    new SphereGeometry(0.5, 6, 4),
    new MeshBasicMaterial({ transparent: true, opacity: DUST_OPACITY, depthWrite: false }),
    { scaleOf: (size, life) => size * Math.min(life / DUST_FADE_SHARE, 1) },
  );
  // The discs find the surface in the View's own track mesh; the bridge
  // offers no surface query and Presentation never asks the sim.
  const surface = trackProbe(track.scene, kartSize.height / 2);
  const shadows = createKartShadows(kartCount, surface);
  const shadowMesh = shadowDiscMesh(shadows, kartSize);
  world.add(
    marks.mesh,
    sparkMesh.mesh,
    sparkHaloMesh.mesh,
    starSparkleMesh.mesh,
    flameMesh.mesh,
    windMesh.mesh,
    dustMesh.mesh,
    shadowMesh.mesh,
  );

  const boxes = itemBoxMeshes(itemBoxes);
  world.add(boxes.mesh);

  const drawnCoins = coinMeshes(coins);
  world.add(drawnCoins.mesh);
  const kartHead = kartModel.scene.getObjectByName("head");
  if (kartHead === undefined) throw new Error("kart.glb carries the driver's head");
  const pits = pitBoxMeshes(pitBoxes, kartSize, tuning, { head: kartHead });
  world.add(pits.mesh);
  const spray = coinSpray();
  world.add(spray.mesh);
  /** Each kart's coins as of the last frame; null before its first. */
  const coinsBefore: (number | null)[] = Array<number | null>(kartCount).fill(null);
  let clockSeconds = 0;

  const cones = meshPool(world, hazardConeMesh(), 0);
  const drones = homingDrones(world);
  const rings = hornRings(world, tuning.hornRadiusM, surface, kartSize.height);
  const runaways = runawayWheels(world, track.scene, tuning.wheelRadiusM);
  const motion = sceneryMotion(track.scene);

  const labels = Array.from({ length: kartCount }, (_, slot) => {
    const element = document.createElement("div");
    element.className = "kart-label";
    // The plate is the kart's key colour, so the name and the paint are read together.
    plate(element, tonesOf(livery(slot).key));
    const label = new CSS2DObject(element);
    label.visible = false;
    world.add(label);
    return label;
  });
  /** Whether each kart's label wore the star's shine last frame, so the key colour is put back once. */
  const labelShone: boolean[] = Array<boolean>(kartCount).fill(false);

  const resize = () => {
    renderer.setSize(container.clientWidth, container.clientHeight);
    labelRenderer.setSize(container.clientWidth, container.clientHeight);
    camera.aspect = container.clientWidth / container.clientHeight;
    camera.updateProjectionMatrix();
  };
  window.addEventListener("resize", resize);

  const target = new Vector3();
  const spin = new Quaternion();
  const up = new Vector3(0, 1, 0);
  /** The kart's own +X: a positive turn about it lifts the nose, along -Z. */
  const sideways = new Vector3(1, 0, 0);
  return {
    update(race, pose, frameSeconds) {
      const { karts: readouts, itemBoxesPresent, coinsPresent, hazards, projectiles } = race;
      const countingDown = race.phase.kind === "countdown";
      clockSeconds += frameSeconds;
      boxes.show(itemBoxesPresent);
      boxes.turn(clockSeconds);
      drawnCoins.show(coinsPresent);
      drawnCoins.turn(clockSeconds);
      readouts.forEach((readout, slot) => {
        // Only a spin-out lowers the count, so a fall is its loss.
        const lost = (coinsBefore[slot] ?? readout.coins) - readout.coins;
        if (lost > 0) spray.burst(readout.position, lost);
        coinsBefore[slot] = readout.coins;
        const drawn = karts[slot];
        if (drawn === undefined) {
          return;
        }
        const { kart, wheels, head, sway } = drawn;
        wheels.turn(readout.speed, frameSeconds);
        sway.state = swayHead(sway.state, sway.last, readout, frameSeconds);
        sway.last = readout;
        if (head !== undefined) {
          head.rotation.z = sway.state.roll;
          // On the mark the driver's eyes are on the lollipop, off the nose on the building's side.
          head.rotation.y =
            -(pitBoxes[slot]?.buildingSide ?? 1) * DRIVER_LOOK_RAD * driverLook(readout.stop);
        }
        kart.position.set(...readout.position);
        kart.quaternion.set(...readout.rotation);
        const onJacks = kartLift(readout.stop, tuning.stopCleanS, kartSize.length);
        if (onJacks.liftM > 0) {
          kart.position.y += onJacks.liftM;
          kart.quaternion.multiply(spin.setFromAxisAngle(sideways, onJacks.pitch));
        }
        if (readout.spinOutS !== null) {
          kart.quaternion.multiply(
            spin.setFromAxisAngle(up, spinOutAngle(readout.spinOutS, tuning.spinOutS)),
          );
        }
        sparkEmitters[slot]?.emit(readout, frameSeconds, countingDown);
        dustEmitters[slot]?.emit(readout, frameSeconds);
        // The star's shine, from the run's seconds every client agrees on:
        // every material pulled toward the colour of the moment, the halo
        // in it and the sparkles off the bodywork; the materials are the
        // kart's own again once the run ends.
        const shine = starShine(readout.starS, viewer.reducedMotion);
        if (shine !== null) {
          shineKart(drawn.model, shine.colour, shine.pull);
          drawn.halo.visible = shine.pull > 0;
          drawn.haloMaterial.color.setHex(shine.colour);
          drawn.haloMaterial.opacity = STAR_HALO_OPACITY * shine.pull;
          if (!viewer.reducedMotion) starSparkles.emit(readout, shine.colour, frameSeconds);
          drawn.shining = true;
        } else if (drawn.shining) {
          shineKart(drawn.model, null, 0);
          drawn.halo.visible = false;
          drawn.shining = false;
        }
        // A spin-out within reach of a star kart is the star's hit: a burst
        // of its sparkles marks it where the kart stands, which covers the
        // metres the interpolation's delay leaves between the two.
        const spunOut = readout.spinOutS !== null;
        if (spunOut && !drawn.spunOut && !viewer.reducedMotion) {
          const star = readouts.find(
            (other) => other.starS !== null && groundDistance(other, readout) <= STAR_HIT_REACH_M,
          );
          const shineOfStar = starShine(star?.starS ?? null, false);
          if (shineOfStar !== null) starSparkles.burst(readout.position, shineOfStar.colour);
        }
        drawn.spunOut = spunOut;
      });
      const own = readouts[viewer.localKart];
      if (own !== undefined) {
        windLines.emit(own, frameSeconds);
        ownPosition = own.position;
      }
      wheelTells.emit(race.wheels, frameSeconds);
      windBrightness = windFade(own?.boostS ?? null, own?.slipstream ?? null);
      tyreMarks.update(readouts, frameSeconds);
      pits.show(readouts, pose.position, frameSeconds);
      spray.update(frameSeconds);
      sparks.update(frameSeconds);
      starSparkleParticles.update(frameSeconds);
      dust.update(frameSeconds);
      wind.update(frameSeconds);
      shadows.update(readouts);
      flames.update(readouts);
      marks.update();
      sparkMesh.update();
      sparkHaloMesh.update();
      starSparkleMesh.update();
      dustMesh.update();
      windMesh.update();
      shadowMesh.update();
      flameMesh.update(frameSeconds);
      cones.show(hazards);
      drones.show(projectiles, (projectile) => readouts[projectile.target]?.position, frameSeconds);
      rings.show(race.blasts);
      runaways.show(race.wheels, frameSeconds);
      motion.update(performance.now());
      for (const label of labels) {
        label.visible = false;
      }
      for (const { kart, text, anchor } of labelsFor(readouts)) {
        const label = labels[kart];
        if (label === undefined) {
          continue;
        }
        label.visible = true;
        label.position.set(...anchor);
        label.element.textContent = text;
        // A star kart's plate underline runs the loop, so the others see it
        // coming; the plate keeps the kart's key colour.
        const shine = starShine(readouts[kart]?.starS ?? null, viewer.reducedMotion);
        if (shine !== null) {
          label.element.style.setProperty("--star", hexCss(shine.colour));
          label.element.dataset.star = "";
          labelShone[kart] = true;
        } else if (labelShone[kart]) {
          label.element.style.removeProperty("--star");
          delete label.element.dataset.star;
          labelShone[kart] = false;
        }
      }
      camera.position.set(...pose.position);
      camera.lookAt(target.set(...pose.target));
      if (camera.fov !== pose.fieldOfView) {
        camera.fov = pose.fieldOfView;
        camera.updateProjectionMatrix();
      }
    },
    render: () => {
      renderer.render(world, camera);
      labelRenderer.render(world, camera);
    },
    presentation(blend) {
      sun.intensity = SUN_INTENSITY * blend.light;
      ambient.intensity = AMBIENT_INTENSITY * blend.light;
      ambient.color.copy(WHITE).lerp(LAMP_COLOUR, blend.lampGlow * LAMP_AMBIENT_TINT);
      lampLight.intensity = LAMP_INTENSITY * blend.lampGlow;
      if (ownPosition !== undefined) {
        const [x, y, z] = ownPosition;
        lampLight.position.set(x, y + LAMP_HEIGHT_M, z);
      }
    },
    dispose() {
      window.removeEventListener("resize", resize);
      renderer.dispose();
      renderer.domElement.remove();
      labelRenderer.domElement.remove();
    },
  };
}

type Position = readonly [number, number, number];

interface RunawayWheels {
  /**
   * Roll each crossing's wheel to where `wheels` has it, or stand it on the
   * wall it will roll in from through the warning, pose its marshal, and
   * light its beacon and flash its hazard zone while the flag is up.
   */
  show(wheels: readonly Wheel[], frameSeconds: number): void;
}

/**
 * A runaway wheel `radius` in size per crossing in `world`, and the
 * marshal and a beacon on each marshal post the track's glTF stands beside
 * one, crossing and post matched by their number. The wheel turns by the
 * ground it covers and bounces as it rolls, its flames flickering off its
 * back; the marshal faces the track, toward the post's lamp. Through the
 * warning the wheel rocks, burning, on the wall top, where no kart
 * reaches it.
 */
function runawayWheels(world: ThreeScene, track: Object3D, radius: number): RunawayWheels {
  const posts = marshalPosts(track);
  const zones = hazardZones(track);
  // No reach above the point asked, so asked from over the wall it finds the wall's top, not the road.
  const wallTop = trackProbe(track, 0);
  const template = runawayWheelMesh(radius);
  const flamesTemplate = runawayWheelFlames(radius);
  interface Drawn {
    readonly heading: Group;
    readonly spin: Object3D;
    /** Not turned with the heading, so the flames point along the tells' back. */
    readonly flames: Object3D;
    /** Who and what stands on the crossing's marshal post, when it has one. */
    readonly postFigures: { readonly marshal: Marshal; readonly beacon: Beacon } | undefined;
    /** Where the wheel was last frame; unset while it is off the course. */
    last: Vector3 | undefined;
  }
  const drawn: Drawn[] = [];
  const drawnAt = (crossing: number): Drawn => {
    const made = drawn[crossing];
    if (made !== undefined) return made;
    const heading = new Group();
    const spin = template.clone();
    heading.add(spin);
    const flames = flamesTemplate.clone();
    world.add(heading, flames);
    const placed = posts.get(crossing);
    let postFigures: Drawn["postFigures"];
    if (placed !== undefined) {
      postFigures = { marshal: marshalMesh(), beacon: marshalBeacon() };
      // The marshal faces along the stand's +z, turned toward the track.
      const stand = new Group();
      stand.position.copy(placed.top);
      stand.rotation.y = Math.atan2(placed.inward.x, placed.inward.z);
      postFigures.marshal.figure.position.x = MARSHAL_OFF_MIDDLE_M;
      postFigures.beacon.mesh.position.set(BEACON_CORNER_M, 0, BEACON_CORNER_M);
      stand.add(postFigures.marshal.figure, postFigures.beacon.mesh);
      world.add(stand);
    }
    const entry: Drawn = { heading, spin, flames, postFigures, last: undefined };
    drawn[crossing] = entry;
    return entry;
  };
  const at = new Vector3();
  const back: Vec3 = [0, 0, 0];
  const behind = new Vector3();
  const backward = new Vector3();
  /** Seconds the View has shown: the flags' wave and the flames' flicker run on it. */
  let clockSeconds = 0;
  return {
    show(wheels, frameSeconds) {
      clockSeconds += frameSeconds;
      zones.show(wheels, clockSeconds);
      wheels.forEach((wheel, crossing) => {
        const entry = drawnAt(crossing);
        entry.postFigures?.marshal.pose(wheel.kind, clockSeconds);
        entry.postFigures?.beacon.show(wheel.kind, clockSeconds);
        entry.heading.visible = wheel.kind !== "quiet";
        entry.flames.visible = wheel.kind !== "quiet";
        if (wheel.kind === "quiet") {
          entry.last = undefined;
          return;
        }
        wheelBack(wheel.yaw, back);
        if (wheel.kind === "warning") {
          // Back from the way it will roll is out over the wall.
          at.set(...wheel.position).addScaledVector(backward.set(...back), WHEEL_WAITS_PAST_FACE_M);
          const top = wallTop.under(at.x, at.y + WALL_PROBE_FROM_M, at.z);
          entry.heading.position.set(at.x, (top?.point[1] ?? at.y) + radius, at.z);
          entry.spin.rotation.x =
            Math.sin(clockSeconds * WHEEL_WAIT_ROCK_RAD_PER_S) * WHEEL_WAIT_ROCK_RAD;
          entry.last = undefined;
        } else {
          at.set(...wheel.position);
          // Rolling toward its -z, the wheel turns back about its axle by the
          // ground it covered since the last frame.
          if (entry.last !== undefined) {
            entry.spin.rotation.x -= at.distanceTo(entry.last) / radius;
          }
          entry.last = at.clone();
          entry.heading.position.set(
            at.x,
            at.y + radius + runawayWheelBounce(wheel.legFraction),
            at.z,
          );
        }
        entry.heading.rotation.y = wheel.yaw;
        entry.flames.position.copy(entry.heading.position);
        entry.flames.lookAt(behind.copy(entry.flames.position).add(backward.set(...back)));
        entry.flames.children.forEach((tongue, index) => {
          tongue.scale.z =
            1 +
            WHEEL_FLAME_FLICKER_SHARE *
              Math.sin(
                clockSeconds * WHEEL_FLAME_FLICKER_RAD_PER_S + index * WHEEL_FLAME_FLICKER_STEP_RAD,
              );
        });
      });
    },
  };
}

/** Where a marshal post's marshal stands, and the level way from the post toward the track. */
interface MarshalPost {
  readonly top: Vector3;
  readonly inward: Vector3;
}

/**
 * Each marshal post in the track's glTF, by its number: the top of its box,
 * where the marshal stands, and the level direction from the post to its lamp,
 * toward the track. The loader drops the dots from the exported names.
 */
function marshalPosts(track: Object3D): Map<number, MarshalPost> {
  const parts = new Map<number, { post?: Box3; lamp?: Box3 }>();
  track.traverse((node) => {
    const match = /^MarshalPost\.?(\d+)\.?(board|chevron)$/.exec(node.name);
    if (match === null) return;
    const number = Number(match[1]);
    const entry = parts.get(number) ?? {};
    const bounds = new Box3().setFromObject(node);
    if (match[2] === "board") entry.post = bounds;
    else entry.lamp = bounds;
    parts.set(number, entry);
  });
  const posts = new Map<number, MarshalPost>();
  for (const [number, { post, lamp }] of parts) {
    if (post === undefined || lamp === undefined) continue;
    const centre = post.getCenter(new Vector3());
    const inward = lamp.getCenter(new Vector3()).sub(centre).setY(0).normalize();
    posts.set(number, { top: new Vector3(centre.x, post.max.y, centre.z), inward });
  }
  return posts;
}

interface HomingDrones {
  /**
   * A drone at each of `projectiles`, `PROJECTILE_HEIGHT_M` over it, turned
   * level to face the point `facing` names for it and tilted into its turn
   * `frameSeconds` on; the rest hidden.
   */
  show(
    projectiles: readonly Projectile[],
    facing: (projectile: Projectile) => Position | undefined,
    frameSeconds: number,
  ): void;
}

/**
 * The homing drones in `world`, made as they are needed and hidden, never
 * removed, when not. Unlike `meshPool` it keeps each drone's flight from
 * frame to frame, so a tilt follows the drone that earned it.
 */
function homingDrones(world: ThreeScene): HomingDrones {
  const drawn: HomingDrone[] = [];
  let flights: DroneFlight[] = [];
  /** Seconds the View has shown: the rotors, the tally and the bob run on it. */
  let clockSeconds = 0;
  return {
    show(projectiles, facing, frameSeconds) {
      clockSeconds += frameSeconds;
      flights = flyDrones(
        flights,
        projectiles.map((projectile) => {
          const [x, , z] = projectile.position;
          const target = facing(projectile);
          const yaw = target === undefined ? undefined : Math.atan2(target[0] - x, target[2] - z);
          return { position: projectile.position, yaw };
        }),
        frameSeconds,
      );
      while (drawn.length < flights.length) {
        const drone = homingDroneMesh();
        world.add(drone.figure);
        drawn.push(drone);
      }
      drawn.forEach((drone, index) => {
        const flight = flights[index];
        drone.figure.visible = flight !== undefined;
        if (flight !== undefined) {
          const [x, y, z] = flight.position;
          drone.figure.position.set(x, y + PROJECTILE_HEIGHT_M, z);
          drone.figure.rotation.y = flight.yaw;
          drone.fly(flight.tilt, clockSeconds);
        }
      });
    },
  };
}

interface MeshPool {
  /**
   * One copy at each of `placed`, `lift` metres over its position and, when
   * `facing` names a point, turned level to face it; the rest hidden.
   */
  show<Placed extends { readonly position: Position }>(
    placed: readonly Placed[],
    facing?: (item: Placed) => Position | undefined,
  ): void;
}

/** The horn blasts' rings in `world`, made as they are needed and hidden, never removed. */
interface HornRings {
  /** Show one ring per blast, grown and faded by the blast's time. */
  show(blasts: readonly Blast[]): void;
}

/**
 * The rings for a blast's reach of `radius`: each a flat ring of unit outer
 * radius laid on the road `surface` finds under the kart's centre, on the
 * road's slope, or where the kart's underside was when nothing is found,
 * scaled out to the reach over the ring's growth and faded over the
 * blast's time. The road is read once per blast, as the ring appears. Unlit
 * and double sided, so it reads from the chase camera on every surface.
 */
function hornRings(
  world: ThreeScene,
  radius: number,
  surface: SurfaceProbe,
  kartHeight: number,
): HornRings {
  const rings: { mesh: Mesh<RingGeometry, MeshBasicMaterial>; blast: number | null }[] = [];
  const geometry = new RingGeometry(1 - HORN_RING_WIDTH_M / radius, 1, 48);
  geometry.rotateX(-Math.PI / 2);
  const up = new Vector3(0, 1, 0);
  const normal = new Vector3();
  return {
    show(blasts) {
      while (rings.length < blasts.length) {
        const material = new MeshBasicMaterial({
          color: HAZARD_ORANGE.mid,
          transparent: true,
          opacity: HORN_RING_OPACITY,
          depthWrite: false,
          side: DoubleSide,
        });
        const mesh = new Mesh(geometry, material);
        world.add(mesh);
        rings.push({ mesh, blast: null });
      }
      rings.forEach((ring, index) => {
        const blast = blasts[index];
        ring.mesh.visible = blast !== undefined;
        if (blast === undefined) {
          ring.blast = null;
          return;
        }
        if (ring.blast !== blast.id) {
          ring.blast = blast.id;
          const [x, y, z] = blast.position;
          const road = surface.under(x, y, z);
          normal.set(...(road?.normal ?? [0, 1, 0]));
          const [rx, ry, rz] = road?.point ?? [x, y - kartHeight / 2, z];
          ring.mesh.position.set(rx, ry, rz).addScaledVector(normal, HORN_RING_LIFT_M);
          ring.mesh.quaternion.setFromUnitVectors(up, normal);
        }
        const grown = Math.min(blast.elapsedS / HORN_RING_GROW_S, 1);
        ring.mesh.scale.setScalar(Math.max(radius * grown, 1e-3));
        const fade = Math.max(1 - blast.elapsedS / BLAST_S, 0);
        ring.mesh.material.opacity = HORN_RING_OPACITY * fade;
      });
    },
  };
}

/**
 * Copies of `template` in `world`, sharing its geometries and materials,
 * made as they are needed and hidden, never removed, when not, so a race of
 * many never churns the scene graph.
 */
function meshPool(world: ThreeScene, template: Object3D, lift: number): MeshPool {
  const meshes: Object3D[] = [];
  const toward = new Vector3();
  return {
    show(placed, facing) {
      while (meshes.length < placed.length) {
        const mesh = template.clone();
        world.add(mesh);
        meshes.push(mesh);
      }
      meshes.forEach((mesh, index) => {
        const item = placed[index];
        mesh.visible = item !== undefined;
        if (item !== undefined) {
          const [x, y, z] = item.position;
          mesh.position.set(x, y + lift, z);
          const target = facing?.(item);
          if (target !== undefined) {
            mesh.lookAt(toward.set(target[0], y + lift, target[2]));
          }
        }
      });
    },
  };
}

/**
 * A four-point glint for the star's sparkles: two crossed stars, one flat
 * and one upright, so it reads from most angles without facing the camera.
 */
function glintGeometry(): BufferGeometry {
  const rim = glint(0, 0, 0.5);
  const point = ([x, y]: Point, flat: boolean): Vector3 =>
    flat ? new Vector3(x, 0, y) : new Vector3(x, y, 0);
  const points: Vector3[] = [];
  for (const flat of [true, false]) {
    rim.forEach((corner, index) => {
      const next = rim[(index + 1) % rim.length] ?? corner;
      points.push(point([0, 0], flat), point(corner, flat), point(next, flat));
    });
  }
  return new BufferGeometry().setFromPoints(points);
}

/**
 * One kart on the grid: `model`, already in its livery, fitted into the box
 * the sim collides with. Its tells are drawn by the View's shared meshes
 * from the transform, not hung on the kart; the star's halo alone rides it.
 */
function kartPresentation(model: Object3D, fit: KartFit): Group {
  const kart = new Group();
  model.scale.setScalar(fit.scale);
  model.position.set(0, fit.groundY, fit.centreZ);
  kart.add(model);
  return kart;
}
