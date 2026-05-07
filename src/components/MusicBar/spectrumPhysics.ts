export interface SpectrumBand {
  height: number;
  target: number;
  velocity: number;
  peak: number;
  peakHoldTime: number;
}

export interface PhysicsOptions {
  rise: number;
  decay: number;
  damping: number;
  peakHoldMs: number;
  peakFall: number;
}

export const DEFAULT_PHYSICS: PhysicsOptions = {
  rise: 74,
  decay: 52,
  damping: 13,
  peakHoldMs: 280,
  peakFall: 0.44
};

export function createBands(count: number): SpectrumBand[] {
  return Array.from({ length: count }, () => ({
    height: 0.035,
    target: 0.035,
    velocity: 0,
    peak: 0.035,
    peakHoldTime: 0
  }));
}

export function stepSpectrum(
  bands: SpectrumBand[],
  targets: number[],
  deltaSeconds: number,
  options = DEFAULT_PHYSICS
) {
  const dt = Math.min(0.04, Math.max(0.001, deltaSeconds));

  for (let index = 0; index < bands.length; index += 1) {
    const band = bands[index];
    const target = Math.max(0.025, Math.min(1, targets[index] ?? 0.025));
    const stiffness = target > band.height ? options.rise : options.decay;

    band.target = target;
    band.velocity = (band.velocity + (target - band.height) * stiffness * dt) * (1 - options.damping * dt);
    band.height = Math.max(0.025, Math.min(1, band.height + band.velocity * dt));

    if (band.height >= band.peak) {
      band.peak = band.height;
      band.peakHoldTime = options.peakHoldMs;
    } else if (band.peakHoldTime > 0) {
      band.peakHoldTime = Math.max(0, band.peakHoldTime - dt * 1000);
    } else {
      band.peak = Math.max(band.height, band.peak - options.peakFall * dt);
    }
  }
}
