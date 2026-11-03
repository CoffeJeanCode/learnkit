import { BRICK_UNIT, type Piece } from "../../../lib/rewards";

/** Height of one standard brick layer in px (matches BRICK_UNIT * 0.9 = 27). */
export const ROW_HEIGHT = 27;

/** Height of the baseplate plate at the bottom in px. */
export const BASEPLATE_HEIGHT = 12;

/** Maximum distance in px to magnetically snap onto studs. */
export const SNAP_RADIUS = 38;

export interface AssembledBrick {
  id: string;
  cols: number;
  rows: number;
  cStart: number;
  layerBottom: number;
}

export interface SnapTarget {
  x: number;
  y: number;
  cStart: number;
  layerBottom: number;
  dist: number;
}

export interface GridGeometry {
  numCols: number;
  gridOriginX: number;
  floorTop: number;
}

/** Computes the table's stud grid layout for a given canvas size. */
export function getGridGeometry(width: number, height: number): GridGeometry {
  const numCols = Math.max(1, Math.floor(width / BRICK_UNIT));
  const gridOriginX = (width - numCols * BRICK_UNIT) / 2;
  const floorTop = height - BASEPLATE_HEIGHT;
  return { numCols, gridOriginX, floorTop };
}

/**
 * Builds an occupancy map of unit cells `col,layer` occupied by assembled bricks.
 * Cells are identified by key `${col},${layer}` mapping to brick ID.
 */
export function buildOccupancyGrid(
  numCols: number,
  bricks: readonly AssembledBrick[],
  ignoreId?: string
): Map<string, string> {
  const grid = new Map<string, string>();
  for (const b of bricks) {
    if (b.id === ignoreId) continue;
    for (let c = b.cStart; c < b.cStart + b.cols && c < numCols; c++) {
      for (let l = b.layerBottom; l < b.layerBottom + b.rows; l++) {
        grid.set(`${c},${l}`, b.id);
      }
    }
  }
  return grid;
}

/**
 * Checks if a brick with `cols` and `rows` can legally snap at column `cStart` and `layerBottom`.
 * Conditions:
 * 1. Fully within horizontal bounds [0, numCols].
 * 2. layerBottom >= 0.
 * 3. None of its cells collide with already assembled bricks.
 * 4. Solid support under at least one stud:
 *    - Baseplate floor if layerBottom === 0.
 *    - An assembled brick at layerBottom - 1 if layerBottom > 0.
 */
export function isValidPlacement(
  cols: number,
  rows: number,
  cStart: number,
  layerBottom: number,
  numCols: number,
  grid: Map<string, string>
): boolean {
  if (cStart < 0 || cStart + cols > numCols || layerBottom < 0) return false;

  // Collision check
  for (let c = cStart; c < cStart + cols; c++) {
    for (let l = layerBottom; l < layerBottom + rows; l++) {
      if (grid.has(`${c},${l}`)) return false;
    }
  }

  // Baseplate floor is always solid support
  if (layerBottom === 0) return true;

  // Stacking check: must rest on at least 1 stud from layer below
  let supportedStuds = 0;
  for (let c = cStart; c < cStart + cols; c++) {
    if (grid.has(`${c},${layerBottom - 1}`)) {
      supportedStuds++;
    }
  }

  return supportedStuds > 0;
}

/**
 * Searches near `(bodyX, bodyY)` for the closest legal stud snap target.
 * Returns the exact snapped coordinates and grid offsets, or null if out of range.
 */
export function findSnapTarget(
  bodyX: number,
  bodyY: number,
  piece: Pick<Piece, "cols" | "rows">,
  width: number,
  height: number,
  assembled: readonly AssembledBrick[],
  draggedId?: string
): SnapTarget | null {
  const { numCols, gridOriginX, floorTop } = getGridGeometry(width, height);
  if (numCols < piece.cols) return null;

  const grid = buildOccupancyGrid(numCols, assembled, draggedId);

  const approxC = (bodyX - gridOriginX) / BRICK_UNIT - piece.cols / 2;
  const centerC = Math.round(approxC);

  const approxLayer = (floorTop - bodyY) / ROW_HEIGHT - piece.rows / 2;
  const centerLayer = Math.max(0, Math.round(approxLayer));

  let best: SnapTarget | null = null;
  let bestDist = Infinity;

  // Search local neighborhood around pointer
  const colDeltas = [0, -1, 1, -2, 2];
  const layerDeltas = [0, 1, -1, 2, -2];

  for (const dc of colDeltas) {
    const c = centerC + dc;
    if (c < 0 || c + piece.cols > numCols) continue;

    for (const dl of layerDeltas) {
      const l = centerLayer + dl;
      if (l < 0) continue;

      if (!isValidPlacement(piece.cols, piece.rows, c, l, numCols, grid)) {
        continue;
      }

      const snapX = gridOriginX + (c + piece.cols / 2) * BRICK_UNIT;
      const snapY = floorTop - (l + piece.rows / 2) * ROW_HEIGHT;
      const dist = Math.hypot(bodyX - snapX, bodyY - snapY);

      if (dist <= SNAP_RADIUS && dist < bestDist) {
        bestDist = dist;
        best = {
          x: snapX,
          y: snapY,
          cStart: c,
          layerBottom: l,
          dist,
        };
      }
    }
  }

  return best;
}

/**
 * Traces structural integrity from the baseplate upward.
 * Returns the set of brick IDs that are firmly grounded to the baseplate.
 * Any brick not in this set has lost its foundation and should drop.
 */
export function findSupportedBricks(bricks: readonly AssembledBrick[]): Set<string> {
  const supported = new Set<string>();
  const queue: AssembledBrick[] = [];

  // Seed with baseplate layer 0 bricks
  for (const b of bricks) {
    if (b.layerBottom === 0) {
      supported.add(b.id);
      queue.push(b);
    }
  }

  // Populate grid of confirmed grounded bricks
  const grid = new Map<string, string>();
  for (const b of queue) {
    for (let c = b.cStart; c < b.cStart + b.cols; c++) {
      for (let l = b.layerBottom; l < b.layerBottom + b.rows; l++) {
        grid.set(`${c},${l}`, b.id);
      }
    }
  }

  // Propagate upward until no new bricks can be grounded
  let changed = true;
  while (changed) {
    changed = false;
    for (const b of bricks) {
      if (supported.has(b.id)) continue;
      let hasSupport = false;
      for (let c = b.cStart; c < b.cStart + b.cols; c++) {
        if (grid.has(`${c},${b.layerBottom - 1}`)) {
          hasSupport = true;
          break;
        }
      }
      if (hasSupport) {
        supported.add(b.id);
        for (let c = b.cStart; c < b.cStart + b.cols; c++) {
          for (let l = b.layerBottom; l < b.layerBottom + b.rows; l++) {
            grid.set(`${c},${l}`, b.id);
          }
        }
        changed = true;
      }
    }
  }

  return supported;
}

/**
 * Tests whether a saved placement `(x, y, angle)` matches an assembled position on the grid.
 * Used when loading from disk or when a brick settles.
 */
export function detectAssembledGridSlot(
  x: number,
  y: number,
  angle: number,
  piece: Pick<Piece, "cols" | "rows">,
  width: number,
  height: number,
  tolerancePx = 6
): { cStart: number; layerBottom: number; snapX: number; snapY: number } | null {
  if (Math.abs(angle) > 0.08) return null;

  const { numCols, gridOriginX, floorTop } = getGridGeometry(width, height);
  if (numCols < piece.cols) return null;

  const cStart = Math.round((x - gridOriginX) / BRICK_UNIT - piece.cols / 2);
  const layerBottom = Math.round((floorTop - y) / ROW_HEIGHT - piece.rows / 2);

  if (cStart < 0 || cStart + piece.cols > numCols || layerBottom < 0) return null;

  const snapX = gridOriginX + (cStart + piece.cols / 2) * BRICK_UNIT;
  const snapY = floorTop - (layerBottom + piece.rows / 2) * ROW_HEIGHT;

  if (Math.hypot(x - snapX, y - snapY) <= tolerancePx) {
    return { cStart, layerBottom, snapX, snapY };
  }

  return null;
}
